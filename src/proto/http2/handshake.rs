use crate::{Connection, ProtoError, ProtoResult};

use super::{codec::Codec, Builder};
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{ready, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use webparse::{http::http2::HTTP2_MAGIC, Binary, Buf};

pub struct Handshake<T> {
    /// 默认参数
    builder: Builder,
    /// 当前握手状态
    state: Handshaking<T>,
    /// 握手日志信息
    span: tracing::Span,
}

/// 握手状态
enum Handshaking<T> {
    /// 协议升级信息写入
    Flushing(Flush<T>),
    /// 等待读取Magic信息
    ReadingPreface(ReadPreface<T>),
    /// 已完成握手, 不可重复握手
    Done,
}

/// Flush a Sink
struct Flush<T> {
    codec: Option<Codec<T>>,
}

/// Read the client connection preface
struct ReadPreface<T> {
    codec: Option<Codec<T>>,
    pos: usize,
}

impl<T> ReadPreface<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(codec: Codec<T>) -> Self {
        Self {
            codec: Some(codec),
            pos: 0,
        }
    }

    fn inner_mut(&mut self) -> &mut T {
        self.codec.as_mut().unwrap().get_mut()
    }
}

impl<T> Future for Handshake<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Output = ProtoResult<Connection<T>>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        loop {
            match &mut self.state {
                Handshaking::Flushing(flush) => {
                    let codec = match Pin::new(flush).poll(cx)? {
                        Poll::Pending => {
                            tracing::trace!(flush.poll = %"Pending");
                            return Poll::Pending;
                        }
                        Poll::Ready(flushed) => {
                            tracing::trace!(flush.poll = %"Ready");
                            flushed
                        }
                    };
                    self.state = Handshaking::ReadingPreface(ReadPreface::new(codec));
                }
                Handshaking::ReadingPreface(read) => {
                    let codec = ready!(Pin::new(read).poll(cx)?);

                    self.state = Handshaking::Done;

                    tracing::trace!("connection established!");
                    let mut c = Connection::new_by_codec(codec);

                    return Poll::Ready(Ok(c));
                }
                Handshaking::Done => {
                    panic!("Handshaking::poll() called again after handshaking was complete")
                }
            }
        }
    }
}

impl<T> Future for Flush<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Output = ProtoResult<Codec<T>>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}

impl<T> Future for ReadPreface<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Output = ProtoResult<Codec<T>>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut buf = [0; 24];
        let mut rem = HTTP2_MAGIC.len() - self.pos;

        while rem > 0 {
            let mut buf = ReadBuf::new(&mut buf[..rem]);
            ready!(Pin::new(self.inner_mut()).poll_read(cx, &mut buf)).map_err(ProtoError::from)?;
            let n = buf.filled().len();
            if n == 0 {
                return Poll::Ready(Err(ProtoError::from(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed before reading preface",
                ))));
            }

            if &HTTP2_MAGIC[self.pos..self.pos + n] != buf.filled() {
                // proto_err!(conn: "read_preface: invalid preface");
                // TODO: Should this just write the GO_AWAY frame directly?
                // return Poll::Ready(Err(Error::library_go_away(Reason::PROTOCOL_ERROR).into()));
            }

            self.pos += n;
            rem -= n;
        }

        Poll::Ready(Ok(self.codec.take().unwrap()))
    }
}

impl<T> Handshake<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn handshake(io: T) -> Handshake<T> {
        let build = Builder::new();
        let codec = Codec::new(io);

        Handshake {
            builder: build,
            state: Handshaking::Flushing(Flush { codec: Some(codec) }),
            span: tracing::trace_span!("server_handshake"),
        }
    }
}
