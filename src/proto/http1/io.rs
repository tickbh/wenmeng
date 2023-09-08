use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use http::request;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::io::poll_read_buf;
use webparse::{BinaryMut, BufMut, Request, Buf};

use crate::{ProtoResult, RecvStream};

pub struct IoBuffer<T> {
    io: T,
    read_buf: BinaryMut,
    write_buf: BinaryMut,
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
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtoResult<()>> {
        println!("poll write!!!!!!!");
        Poll::Pending
        // self.inner.control.poll_write(cx, &mut self.codec)
        // loop {
        //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
        // }
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
        match self.poll_read(cx)? {
            Poll::Ready(0) => return Poll::Ready(Ok(0)),
            Poll::Ready(n) => size += n,
            Poll::Pending => (),
        }
        Poll::Ready(Ok(size))
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtoResult<Request<RecvStream>>>> {
        match ready!(self.poll_read_all(cx)?) {
            // socket被断开, 提前结束
            0 => return Poll::Ready(None),
            // 收到新的消息头, 解析包体消息
            _ => {
                let mut request = Request::new();
                let size = request.parse_buffer(&mut self.read_buf.clone())?;
                if request.is_partial() {
                    return Poll::Pending;
                }
                
                self.read_buf.advance(size);
                let new_request = request.into(RecvStream::empty()).0;
                return Poll::Ready(Some(Ok(new_request)));
            }
        }
    }
}
