use std::{pin::Pin, task::{Context, Poll}, io};

use tokio::io::{AsyncRead, ReadBuf, AsyncWrite};



#[derive(Debug)]
pub struct FramedWrite<T> {
    /// Upstream `AsyncWrite`
    inner: T,
}


impl<T> FramedWrite<T> 
where
    T: AsyncRead + AsyncWrite + Unpin, {
    pub fn new(io: T) -> Self {
        Self { inner: io }
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
} 


impl<T: AsyncRead + Unpin> AsyncRead for FramedWrite<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}