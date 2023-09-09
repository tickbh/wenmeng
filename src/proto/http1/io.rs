use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_util::future::poll_fn;
use http::request;
use tokio::{io::{AsyncRead, AsyncWrite, ReadBuf}, sync::mpsc::Sender};
use tokio_util::io::poll_read_buf;
use webparse::{BinaryMut, BufMut, Request, Buf, WebError, http::http2, Binary, Serialize, Response};

use crate::{ProtoResult, RecvStream, ProtoError};

pub struct IoBuffer<T> {
    io: T,
    read_buf: BinaryMut,
    write_buf: BinaryMut,
    write_sender: Sender<()>,
    read_sender: Option<Sender<(bool, Binary)>>,
    is_builder: bool,
}

impl<T> IoBuffer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T, write_sender: Sender<()>) -> Self {
        Self {
            io: io,
            read_buf: BinaryMut::new(),
            write_buf: BinaryMut::new(),
            write_sender,
            read_sender: None,
            is_builder: false,
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtoResult<()>> {
        println!("!!!!!!!!!!!!!!!!!!!!!!!");








        
        if self.write_buf.is_empty() {
            return Poll::Ready(Ok(()))
        }
        match ready!(Pin::new(&mut self.io).poll_write(cx, &self.write_buf.chunk()))? {
            n => {
                self.write_buf.advance(n);
                if self.write_buf.is_empty() {
                    return Poll::Ready(Ok(()))
                }
            }
        };
        println!("poll write!!!!!!!");
        Poll::Pending
    }

    pub fn poll_read(&mut self, cx: &mut Context<'_>) -> Poll<ProtoResult<usize>> {
        self.read_buf.reserve(1);
        let n = {
            let mut buf = ReadBuf::uninit(self.read_buf.chunk_mut());
            let ptr = buf.filled().as_ptr();
            ready!(Pin::new(&mut self.io).poll_read(cx, &mut buf)?);
            assert_eq!(ptr, buf.filled().as_ptr());
            buf.filled().len()
        };

        unsafe {
            self.read_buf.advance_mut(n);
        }
        Poll::Ready(Ok(n))
    }

    pub fn poll_read_all(&mut self, cx: &mut Context<'_>) -> Poll<ProtoResult<usize>> {
        let mut size = 0;
        loop {
            match self.poll_read(cx)? {
                Poll::Ready(0) => return Poll::Ready(Ok(0)),
                Poll::Ready(n) => size += n,
                Poll::Pending => break,
            }
        }
        Poll::Ready(Ok(size))
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtoResult<Request<RecvStream>>>> {
        let _ = self.poll_write(cx);
        match ready!(self.poll_read_all(cx)?) {
            // socket被断开, 提前结束
            0 => return Poll::Ready(None),
            // 收到新的消息头, 解析包体消息
            _ => {
                if self.is_builder {
                    if let Some(sender) = &self.read_sender {
                        let binary = Binary::from(self.read_buf.chunk().to_vec());
                        if let Ok(_) = sender.try_send((false, binary)) {
                            self.read_buf.advance_all();
                        }
                    }
                    return Poll::Pending;
                }
                let mut request = Request::new();
                println!("data = {:?}", self.read_buf.chunk());
                let size = match request.parse_buffer(&mut self.read_buf.clone()) {
                    Err(e) => {
                        if e.is_partial() {
                            return Poll::Pending;
                        } else {
                            if self.read_buf.remaining() >= http2::MAIGC_LEN && &self.read_buf[..http2::MAIGC_LEN] == http2::HTTP2_MAGIC {
                                self.read_buf.advance(http2::MAIGC_LEN);
                                let err = ProtoError::UpgradeHttp2;
                                return Poll::Ready(Some(Err(err)))
                            }
                            return Poll::Ready(Some(Err(e.into())));
                        }
                    }
                    Ok(n) => n,
                };
                // let size = request.parse_buffer(&mut self.read_buf.clone())?;
                if request.is_partial() {
                    return Poll::Pending;
                }
                
                self.is_builder = true;
                self.read_buf.advance(size);
                let body_len = request.get_body_len();
                let new_request = if request.method().is_nobody() {
                    request.into(RecvStream::empty()).0
                } else {
                    let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
                    self.read_sender = Some(sender);
                    let mut binary = BinaryMut::new();
                    binary.put_slice(self.read_buf.chunk());
                    self.read_buf.advance(self.read_buf.remaining());
                    let is_end = body_len <= binary.remaining();
                    request.into(RecvStream::new(receiver, binary, is_end)).0
                };
                return Poll::Ready(Some(Ok(new_request)));
            }
        }
    }

    pub fn into(self) -> (T, BinaryMut, BinaryMut) {
        (self.io, self.read_buf, self.write_buf)
    }
    
    pub async fn send_response<R: Serialize>(&mut self, mut res: Response<R>) -> ProtoResult<()> {
        // self.io.
        let mut buffer = BinaryMut::new();
        res.serialize(&mut buffer);
        self.write_buf.put_slice(buffer.chunk());
        let _ = poll_fn(|cx| self.poll_write(cx));
        
        Ok(())
    }
}
