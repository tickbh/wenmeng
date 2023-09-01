use std::io::Cursor;

use tokio::io::AsyncWrite;
use webparse::{http::http2::{HeaderIndex, Payload}, BinaryMut, Buf, MarkBuf};

use crate::proto::http2::{FrameSize, DEFAULT_MAX_FRAME_SIZE};

/// Initialize the connection with this amount of write buffer.
///
/// The minimum MAX_FRAME_SIZE is 16kb, so always be able to send a HEADERS
/// frame that big.
const DEFAULT_BUFFER_CAPACITY: usize = 16 * 1_024;

/// Min buffer required to attempt to write a frame
const MIN_BUFFER_CAPACITY: usize = 9 + CHAIN_THRESHOLD;

/// Chain payloads bigger than this. The remote will never advertise a max frame
/// size less than this (well, the spec says the max frame size can't be less
/// than 16kb, so not even close).
const CHAIN_THRESHOLD: usize = 256;

#[derive(Debug)]
pub struct FramedWrite<T, B: Buf+MarkBuf> {
    /// Upstream `AsyncWrite`
    inner: T,

    encoder: Encoder<B>,
}

#[derive(Debug)]
struct Encoder<B: Buf+MarkBuf> {
    /// HPACK encoder
    head_index: HeaderIndex,

    /// Write buffer
    ///
    /// TODO: Should this be a ring buffer?
    buf: Cursor<BinaryMut>,

    // /// Next frame to encode
    next: Option<Next<B>>,

    // /// Last data frame
    // last_data_frame: Option<Data<B>>,
    /// Max frame size, this is specified by the peer
    max_frame_size: FrameSize,

    /// Whether or not the wrapped `AsyncWrite` supports vectored IO.
    is_write_vectored: bool,
}


#[derive(Debug)]
enum Next<B: Buf+MarkBuf> {
    Data(Payload<B>),
    Continuation(Payload<B>),
}

// TODO: Make generic
impl<T, B> FramedWrite<T, B>
where
    T: AsyncWrite + Unpin,
    B: Buf+MarkBuf
{
    pub fn new(inner: T) -> FramedWrite<T, B> {
        let is_write_vectored = inner.is_write_vectored();
        FramedWrite {
            inner,
            encoder: Encoder {
                head_index: HeaderIndex::new(),
                buf: Cursor::new(BinaryMut::with_capacity(DEFAULT_BUFFER_CAPACITY)),
                next: None,
                max_frame_size: DEFAULT_MAX_FRAME_SIZE,
                is_write_vectored,
            },
        }
    }
}
