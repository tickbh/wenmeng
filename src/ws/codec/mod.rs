mod framed_read;
mod framed_write;

use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use algorithm::buf::BinaryMut;
pub use framed_read::FramedRead;
pub use framed_write::FramedWrite;
use futures::Stream;
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::ws::{DataFrameable, OwnedMessage};

use crate::ProtResult;

#[derive(Debug)]
pub struct WsCodec<T> {
    inner: FramedRead<FramedWrite<T>>,
}

impl<T> WsCodec<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Returns a new `Codec` with the default max frame size
    #[inline]
    pub fn new(io: T, is_client: bool) -> Self {
        let framed_write = FramedWrite::new(io);
        let inner = FramedRead::new(framed_write, is_client);
        WsCodec { inner }
    }

    pub fn into_io(self) -> T {
        self.inner.into_io().into_io()
    }

    pub fn is_write_end(&self) -> bool {
        self.inner.get_ref().is_write_end()
    }

    pub fn get_reader(&mut self) -> &mut FramedRead<FramedWrite<T>> {
        &mut self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut().get_mut()
    }

    /// Returns `Ready` when the codec can buffer a frame
    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.framed_write().poll_ready(cx)
    }

    /// Returns `Ready` when the codec can buffer a frame
    pub fn poll_flush(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.framed_write().flush(cx)
    }

    fn framed_write(&mut self) -> &mut FramedWrite<T> {
        self.inner.get_mut()
    }

    pub fn shutdown(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.framed_write().shutdown(cx)
    }

    pub fn set_cache_buf(&mut self, read_buf: BinaryMut, write_buf: BinaryMut) {
        self.inner.set_cache_buf(read_buf);
        self.framed_write().set_cache_buf(write_buf);
    }

    pub fn send_msg(&mut self, msg: OwnedMessage, mask: bool) -> ProtResult<usize> {
        log::trace!("Websocket:发送帧数据: {:?}", msg);
        if mask {
            msg.write_to(self.framed_write().get_mut_bytes(), Some(rand::random()))?;
        } else {
            msg.write_to(self.framed_write().get_mut_bytes(), None)?;
        }
        Ok(0)
    }
}

impl<T> Stream for WsCodec<T>
where
    T: AsyncRead + Unpin,
{
    type Item = ProtResult<OwnedMessage>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}
