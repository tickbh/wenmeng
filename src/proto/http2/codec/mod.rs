mod error;
mod framed_read;
mod framed_write;

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_core::Stream;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::length_delimited;
use webparse::http::http2::encoder::Encoder;
use webparse::http::http2::frame::Frame;
use webparse::http::http2::HeaderIndex;

use crate::ProtoResult;

pub use self::framed_read::FramedRead;
pub use self::framed_write::FramedWrite;

#[derive(Debug)]
pub struct Codec<T> {
    inner: FramedRead<FramedWrite<T>>,
    header_index: HeaderIndex,
}

impl<T> Codec<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Returns a new `Codec` with the default max frame size
    #[inline]
    pub fn new(io: T) -> Self {
        Self::with_max_recv_frame_size(io, super::DEFAULT_MAX_FRAME_SIZE as usize)
    }

    /// Returns a new `Codec` with the given maximum frame size
    pub fn with_max_recv_frame_size(io: T, max_frame_size: usize) -> Self {
        // Wrap with writer
        let framed_write = FramedWrite::new(io);

        // Delimit the frames
        let delimited = length_delimited::Builder::new()
            .big_endian()
            .length_field_length(3)
            .length_adjustment(9)
            .num_skip(0) // Don't skip the header
            .new_read(framed_write);

        let mut inner = FramedRead::new(delimited);

        // Use FramedRead's method since it checks the value is within range.
        // inner.set_max_frame_size(max_frame_size);

        Codec {
            inner,
            header_index: HeaderIndex::new(),
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut().get_mut()
    }

    /// Returns `Ready` when the codec can buffer a frame
    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.framed_write().poll_ready(cx)
    }

    fn framed_write(&mut self) -> &mut FramedWrite<T> {
        self.inner.get_mut()
    }

    pub fn send_frame(&mut self, frame: Frame) {
        // let encoder = Encoder {
        //     index: Arc::new(self.header_index)
        // };

    }
}

impl<T> Stream for Codec<T>
where
    T: AsyncRead + Unpin,
{
    type Item = ProtoResult<Frame>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}
