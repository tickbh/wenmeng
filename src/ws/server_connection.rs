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
// Created Date: 2023/10/07 09:41:03

use std::{
    task::{ready, Context, Poll},
    time::{Duration, Instant},
};

use tokio_stream::Stream;

use tokio::{
    io::{AsyncRead, AsyncWrite},
};
use webparse::{
    ws::{CloseCode, CloseData, OwnedMessage},
    Binary, BinaryMut,
};

use crate::{
    ProtResult, TimeoutLayer,
};

use super::{Control, WsCodec, state::WsState};

pub struct ServerWsConnection<T> {
    codec: WsCodec<T>,
    inner: InnerConnection,
    timeout: Option<TimeoutLayer>,
}

struct InnerConnection {
    state: WsState,
    control: Control,
}

unsafe impl<T> Sync for ServerWsConnection<T> {}

unsafe impl<T> Send for ServerWsConnection<T> {}

impl<T> ServerWsConnection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> ServerWsConnection<T> {
        ServerWsConnection {
            codec: WsCodec::new(io, false),
            inner: InnerConnection {
                state: WsState::Open,
                control: Control::new(),
            },
            timeout: None,
        }
    }

    pub fn into_io(self) -> T {
        self.codec.into_io()
    }

    pub fn set_read_timeout(&mut self, read_timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout
            .as_mut()
            .unwrap()
            .set_read_timeout(read_timeout);
    }

    pub fn set_write_timeout(&mut self, write_timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout
            .as_mut()
            .unwrap()
            .set_write_timeout(write_timeout);
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout.as_mut().unwrap().set_timeout(timeout);
    }

    pub fn set_ka_timeout(&mut self, timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout.as_mut().unwrap().set_ka_timeout(timeout);
    }

    pub fn set_timeout_layer(&mut self, timeout_layer: Option<TimeoutLayer>) {
        self.timeout = timeout_layer;
    }

    pub fn pull_accept(&mut self, _cx: &mut Context<'_>) -> Poll<Option<ProtResult<()>>> {
        Poll::Pending
    }

    pub fn poll_request(&mut self, cx: &mut Context<'_>) -> Poll<Option<ProtResult<OwnedMessage>>> {
        if self.timeout.is_some() {
            let (ready_time, is_read_end, is_write_end, is_idle) = (
                Instant::now(),
                false,
                self.inner.control.is_write_end(&self.codec),
                false,
            );
            self.timeout.as_mut().unwrap().poll_ready(
                cx,
                "server",
                ready_time,
                is_read_end,
                is_write_end,
                is_idle,
            )?;
        }

        self.inner.control.poll_request(cx, &mut self.codec)
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<()>> {
        self.inner.control.poll_write(cx, &mut self.codec)
    }

    // pub async fn handle_request<F>(
    //     &mut self,
    //     addr: &Option<SocketAddr>,
    //     mut r: RecvRequest,
    //     f: &mut F,
    //     middles: &mut Vec<Box<dyn Middleware>>,
    // ) -> ProtResult<Option<bool>>
    // where
    //     F: OperateTrait + Send,
    // {
    //     let stream_id: Option<StreamIdentifier> = r.extensions_mut().remove::<StreamIdentifier>();

    //     let res = HttpHelper::handle_request(Version::Http2, addr, r, f, middles).await?;
    //     self.send_response(res, stream_id.unwrap_or(StreamIdentifier::client_first()))
    //         .await?;
    //     return Ok(None);
    // }

    pub fn set_cache_buf(&mut self, read_buf: BinaryMut, write_buf: BinaryMut) {
        self.codec.set_cache_buf(read_buf, write_buf)
    }

    pub fn set_handshake_status(&mut self, binary: Binary) {
        self.inner.control.set_handshake_status(binary, false)
    }

    pub fn send_owned_message(&mut self, msg: OwnedMessage) -> ProtResult<()> {
        self.inner.control.send_owned_message(msg)
    }
    
    pub fn receiver_close(&mut self, data: Option<CloseData>) -> ProtResult<()> {
        self.inner.state.set_closing(data.unwrap_or(CloseData::normal()));
        Ok(())
    }
}

impl<T> Stream for ServerWsConnection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<OwnedMessage>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        // std::pin::Pin::unpin
        loop {
            match self.inner.state {
                WsState::Open => {
                    match self.poll_request(cx) {
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                        Poll::Ready(Some(Ok(v))) => {
                            return Poll::Ready(Some(Ok(v)));
                        }
                        Poll::Ready(_) => {
                            let close = OwnedMessage::Close(Some(CloseData::new(
                                CloseCode::Abnormal,
                                "network".to_string(),
                            )));
                            return Poll::Ready(Some(Ok(close)));
                        }
                    };
                }
                WsState::Closing(_) => {
                    ready!(self.codec.shutdown(cx))?;
                    ready!(self.poll_write(cx))?;
                    self.inner.state.set_closed(None);
                }
                WsState::Closed(_) => {
                    return Poll::Ready(None);
                }
            }
        }
        Poll::Pending
    }
}
