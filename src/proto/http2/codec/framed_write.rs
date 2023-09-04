use std::{
    io,
    pin::Pin,
    task::{ready, Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use webparse::{BinaryMut, Buf};

use crate::proto::http2::{FrameSize, DEFAULT_MAX_FRAME_SIZE};

#[derive(Debug)]
pub struct FramedWrite<T> {
    /// Upstream `AsyncWrite`
    inner: T,

    binary: BinaryMut,

    max_frame_size: FrameSize,
}

impl<T> FramedWrite<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        Self {
            inner: io,
            binary: BinaryMut::new(),
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn get_bytes(&mut self) -> &mut BinaryMut {
        &mut self.binary
    }

    pub fn has_capacity(&self) -> bool {
        self.binary.remaining() < self.max_frame_size as usize
    }

    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        if !self.has_capacity() {
            // Try flushing
            ready!(self.flush(cx))?;

            if !self.has_capacity() {
                return Poll::Pending;
            }
        }

        Poll::Ready(Ok(()))
    }

    pub fn flush(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let span = tracing::trace_span!("FramedWrite::flush");
        let _e = span.enter();
        if !self.binary.has_remaining() {
            return Poll::Ready(Ok(()));
        }

        let n = ready!(Pin::new(&mut self.inner).poll_write(cx, self.binary.chunk()))?;
        self.binary.advance(n);
        if self.binary.remaining() == 0 && self.binary.cursor() > 10 * self.max_frame_size as usize
        {
            self.binary = BinaryMut::new();
        }
        Poll::Ready(Ok(()))
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
