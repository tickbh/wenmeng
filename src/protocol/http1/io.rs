use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_util::future::poll_fn;
use http::request;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::mpsc::Sender,
};
use tokio_util::io::poll_read_buf;
use webparse::{
    http::http2, Binary, BinaryMut, Buf, BufMut, Request, Response, Serialize, WebError,
};

use crate::{ProtError, ProtResult, RecvStream};

pub struct IoBuffer<T> {
    io: T,
    read_buf: BinaryMut,
    write_buf: BinaryMut,

    inner: ConnectionInfo,
}

struct ConnectionInfo {
    deal_req: usize,
    read_sender: Option<Sender<(bool, Binary)>>,
    res: Option<Response<RecvStream>>,
    is_keep_alive: bool,
    is_send_body: bool,
    is_send_header: bool,
    is_build_req: bool,
    is_send_end: bool,
}

impl ConnectionInfo {

    pub fn is_active_close(&self) -> bool {
        self.is_send_end && !self.is_keep_alive
    }
}

impl<T> IoBuffer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        Self {
            io: io,
            read_buf: BinaryMut::new(),
            write_buf: BinaryMut::new(),

            inner: ConnectionInfo {
                deal_req: 0,
                read_sender: None,
                res: None,
                is_keep_alive: false,
                is_send_body: false,
                is_send_header: false,
                is_build_req: false,
                is_send_end: false,
            },
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<()>> {
        println!("!!!!!!!!!!!!!!!!!!!!!!!");
        if let Some(res) = &mut self.inner.res {
            if !self.inner.is_send_header {
                res.encode_header(&mut self.write_buf)?;
                self.inner.is_send_header = true;
            }

            if !res.body().is_end() || !self.inner.is_send_body {
                self.inner.is_send_body = true;
                let _ = res.body_mut().poll_encode(cx, &mut self.write_buf);
                if res.body().is_end() {
                    self.inner.is_send_end = true;
                    self.inner.deal_req += 1;
                }
            }
        }

        if self.inner.is_send_end {
            self.inner.res = None;
            self.inner.is_build_req = false;
            self.inner.is_send_header = false;
        }

        if self.write_buf.is_empty() {
            return Poll::Ready(Ok(()));
        }
        match ready!(Pin::new(&mut self.io).poll_write(cx, &self.write_buf.chunk()))? {
            n => {
                self.write_buf.advance(n);
                if self.write_buf.is_empty() {
                    return Poll::Ready(Ok(()));
                }
            }
        };
        println!("poll write!!!!!!!");
        Poll::Pending
    }

    pub fn poll_read(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
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

    pub fn poll_read_all(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        let mut size = 0;
        loop {
            match self.poll_read(cx)? {
                Poll::Ready(0) => return Poll::Ready(Ok(0)),
                Poll::Ready(n) => size += n,
                Poll::Pending => {
                    if size == 0 {
                        return Poll::Pending;
                    } else {
                        break;
                    }
                }
            }
        }
        Poll::Ready(Ok(size))
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Request<RecvStream>>>> {
        let _ = self.poll_write(cx)?;
        if self.inner.is_active_close() && self.write_buf.is_empty() {
            println!("test:::: write client end!!!");
            return Poll::Ready(None);
        }
        match ready!(self.poll_read_all(cx)?) {
            // socket被断开, 提前结束
            0 => {
                println!("test:::: recv client end!!!");
                return Poll::Ready(None);
            }
            // 收到新的消息头, 解析包体消息
            _ => {
                if self.inner.is_build_req {
                    if let Some(sender) = &self.inner.read_sender {
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
                            if self.read_buf.remaining() >= http2::MAIGC_LEN
                                && &self.read_buf[..http2::MAIGC_LEN] == http2::HTTP2_MAGIC
                            {
                                self.read_buf.advance(http2::MAIGC_LEN);
                                let err = ProtError::UpgradeHttp2(Binary::new(), None);
                                return Poll::Ready(Some(Err(err)));
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

                self.read_buf.advance(size);
                self.inner.is_send_body = false;
                self.inner.is_send_end = false;
                self.inner.is_build_req = true;
                self.inner.is_keep_alive = request.is_keep_alive();
                let body_len = request.get_body_len();
                let new_request = if request.method().is_nobody() {
                    request.into(RecvStream::empty()).0
                } else {
                    let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
                    self.inner.read_sender = Some(sender);
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

    pub async fn send_response(&mut self, mut res: Response<RecvStream>) -> ProtResult<()> {
        self.inner.res = Some(res);
        // self.io.
        // let mut buffer = BinaryMut::new();
        // let _ = res.encode_header(&mut buffer)?;
        // // let _ = res.body_mut().pol
        // self.write_buf.put_slice(buffer.chunk());
        // let _ = poll_fn(|cx| self.poll_write(cx));
        // if !res.body().is_end() {
        //     self.res = Some(res);
        // }
        Ok(())
    }
}
