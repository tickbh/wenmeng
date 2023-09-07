use crate::{proto::http2::codec::Codec, Builder, Connection, ProtoError, ProtoResult};

use std::{
    future::Future,
    io,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use webparse::{http::http2::HTTP2_MAGIC, Binary, Buf};

pub struct StateHandshake {
    /// 默认参数
    builder: Builder,
    /// 当前握手状态
    state: Handshaking,
    /// 握手日志信息
    span: tracing::Span,
}

/// 握手状态
enum Handshaking {
    /// 还未进行握手, 确定http2协议则开始握手
    None,
    /// 协议升级信息写入
    Flushing(Flush),
    /// 等待读取Magic信息
    ReadingPreface(ReadPreface),
    /// 已完成握手, 不可重复握手
    Done,
}

/// Flush a Sink
struct Flush(Binary);

/// Read the client connection preface
struct ReadPreface {
    pos: usize,
}

impl ReadPreface {
    pub fn new() -> Self {
        ReadPreface { pos: 0 }
    }

    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buf = [0; 24];
        let mut rem = HTTP2_MAGIC.len() - self.pos;

        while rem > 0 {
            let mut buf = ReadBuf::new(&mut buf[..rem]);
            ready!(Pin::new(codec.get_mut()).poll_read(cx, &mut buf)).map_err(ProtoError::from)?;
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
                return Poll::Ready(Err(ProtoError::Extension("handshake not match")));
            }

            self.pos += n;
            rem -= n;
        }

        Poll::Ready(Ok(()))
    }
}

impl StateHandshake {
    pub fn new_server() -> StateHandshake {
        StateHandshake {
            builder: Builder::new(),
            state: Handshaking::None,
            span: tracing::trace_span!("server_handshake"),
        }
    }

    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        loop {
            match &mut self.state {
                Handshaking::None => {
                    self.state = Handshaking::Flushing(Flush(Binary::new()));
                }
                Handshaking::Flushing(flush) => {
                    match ready!(flush.poll_handle(cx, codec)) {
                        Ok(_) => {
                            tracing::trace!(flush.poll = %"Ready");
                            self.state = Handshaking::ReadingPreface(ReadPreface::new());
                            continue;
                        }
                        Err(e) => return Poll::Ready(Err(e)),
                    };
                }
                Handshaking::ReadingPreface(read) => {
                    match ready!(read.poll_handle(cx, codec)) {
                        Ok(_) => {
                            tracing::trace!(flush.poll = %"Ready");
                            self.state = Handshaking::Done;
                            return Poll::Ready(Ok(()));
                        }
                        Err(e) => return Poll::Ready(Err(e)),
                    };
                }
                Handshaking::Done => {
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}

impl Flush {
    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if !self.0.has_remaining() {
            return Poll::Ready(Ok(()));
        }

        // codec.get_mut().pull_write()

        loop {
            match ready!(Pin::new(codec.get_mut()).poll_write(cx, self.0.chunk())) {
                Ok(n) => {
                    self.0.advance(n);
                }
                Err(e) => return Poll::Ready(Err(e.into())),
            }
            if !self.0.has_remaining() {
                return Poll::Ready(Ok(()));
            }
        }
    }
}

unsafe impl Send for StateHandshake {}

unsafe impl Sync for StateHandshake {}
