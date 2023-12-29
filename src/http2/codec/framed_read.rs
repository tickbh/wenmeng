// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
// 
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
// 
// Author: tickbh
// -----
// Created Date: 2023/09/14 09:42:25

use std::pin::Pin;
use std::task::{ready, Poll};

use bytes::{BytesMut, BufMut};
use tokio_stream::Stream;
use tokio::io::AsyncRead;
use tokio_util::codec::{FramedRead as InnerFramedRead};
use tokio_util::codec::{LengthDelimitedCodec};
use webparse::http::http2::frame::{Frame, Kind};
use webparse::http::http2::{frame, Decoder};
use webparse::http2::DEFAULT_SETTINGS_HEADER_TABLE_SIZE;
use webparse::{Binary, BinaryMut, Buf};

use crate::ProtResult;



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
    
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
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
            max_header_list_size: DEFAULT_SETTINGS_HEADER_TABLE_SIZE,
            partial: None,
        }
    }

    pub fn into_io(self) -> T {
        self.inner.into_inner()
    }
    
    pub fn set_cache_buf(&mut self, read_buf: BinaryMut) {
        self.inner.read_buffer_mut().put_slice(read_buf.chunk());
    }
}

impl<T> AsyncRead for FramedRead<T>
where
    T: AsyncRead + Unpin, {
        fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        use bytes::Buf;
        if self.inner.read_buffer_mut().remaining() > 0 {
            let read = std::cmp::min(buf.remaining(), self.inner.read_buffer_mut().remaining());
            buf.put_slice(&self.inner.read_buffer_mut().chunk()[..read]);
            self.inner.read_buffer_mut().advance(read);
            return Poll::Ready(Ok(()))
        }
        Pin::new(self.get_mut().get_mut()).poll_read(cx, buf)
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
                log::trace!("HTTP2:收到帧数据: {:?}", frame);
                return Poll::Ready(Some(Ok(frame)));
            }
        }
    }
}

fn decode_frame(
    decoder: &mut Decoder,
    max_header_list_size: usize,
    partial_inout: &mut Option<Partial>,
    bytes: BytesMut,
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

    let _kind = head.kind();
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
