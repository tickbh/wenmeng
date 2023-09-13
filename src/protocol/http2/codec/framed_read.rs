use std::pin::Pin;
use std::task::{ready, Poll};

use bytes::{BytesMut, BufMut};
use futures_core::Stream;
use tokio::io::AsyncRead;
use tokio_util::codec::{FramedRead as InnerFramedRead, length_delimited};
use tokio_util::codec::{LengthDelimitedCodec, LengthDelimitedCodecError};
use webparse::http::http2::frame::{Frame, Kind};
use webparse::http::http2::{frame, Decoder, HeaderIndex};
use webparse::{Binary, BinaryMut, WebResult, Buf};

use crate::protocol::{ProtError, ProtResult};

use super::FramedWrite;

#[derive(Debug)]
pub struct FramedRead<T> {
    inner: InnerFramedRead<T, LengthDelimitedCodec>,

    decoder: Decoder,

    max_header_list_size: usize,

    partial: Option<Partial>,
}

/// Partially loaded headers frame
#[derive(Debug)]
struct Partial {
    /// Empty frame
    frame: Continuable,

    /// Partial header payload
    buf: BinaryMut,
}

#[derive(Debug)]
enum Continuable {
    Headers(frame::Headers),
    PushPromise(frame::PushPromise),
}


impl<T> FramedRead<T> {
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T> FramedRead<T>
where
    T: AsyncRead + Unpin,
{
    pub fn new(delimited: InnerFramedRead<T, LengthDelimitedCodec>) -> FramedRead<T> {

        FramedRead {
            inner: delimited,
            decoder: Decoder::new(),
            max_header_list_size: 0,
            partial: None,
        }
    }
    
    pub fn set_cache_buf(&mut self, read_buf: BinaryMut) {
        self.inner.read_buffer_mut().put_slice(read_buf.chunk());
    }

}

impl<T> Stream for FramedRead<T>
where
    T: AsyncRead + Unpin,
{
    type Item = ProtResult<Frame<Binary>>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            println!("poll!!!!!!");
            // let xx = Pin::new(&mut self.inner).poll_next(cx);
            // println!("xxx = {:?}", xx);
            let bytes = match ready!(Pin::new(&mut self.inner).poll_next(cx)) {
                Some(Ok(bytes)) => bytes,
                Some(Err(e)) => return Poll::Ready(Some(Err(e.into()))),
                None => return Poll::Ready(None),
            };

            let Self {
                ref mut decoder,
                max_header_list_size,
                ref mut partial,
                ..
            } = *self;
            if let Some(frame) = decode_frame(decoder, max_header_list_size, partial, bytes)? {
                println!("received frame = {:?}", frame);
                return Poll::Ready(Some(Ok(frame)));
            }
        }
    }
}

fn decode_frame(
    decoder: &mut Decoder,
    max_header_list_size: usize,
    partial_inout: &mut Option<Partial>,
    mut bytes: BytesMut,
) -> ProtResult<Option<Frame>> {
    use bytes::Buf;
    let span = tracing::trace_span!("FramedRead::decode_frame", offset = bytes.len());
    let _e = span.enter();

    let mut bytes = Binary::from(bytes.chunk().to_vec());

    tracing::trace!("decoding frame from {}B", bytes.len());

    // Parse the head
    let head = frame::FrameHeader::parse(&mut bytes)?;

    if partial_inout.is_some() && head.kind() != &Kind::Continuation {
        // proto_err!(conn: "expected CONTINUATION, got {:?}", head.kind());
        // return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
    }

    let kind = head.kind();
    let frame = Frame::parse(head, bytes, decoder, max_header_list_size)?;

    Ok(Some(frame))
}
// /// Partially loaded headers frame
// #[derive(Debug)]
// struct Partial {
//     /// Empty frame
//     frame: Continuable,

//     /// Partial header payload
//     buf: BinaryMut,
// }

// #[derive(Debug)]
// enum Continuable {
//     Headers(FrameHeader),
//     PushPromise(PushPromise),
// }
