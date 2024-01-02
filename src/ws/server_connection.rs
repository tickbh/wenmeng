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
    net::SocketAddr,
    task::{ready, Context, Poll},
    time::Duration,
};

use tokio_stream::Stream;

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::{channel, Receiver},
};
use webparse::{
    http::http2::frame::{Reason, StreamIdentifier},
    Binary, BinaryMut, Version,
};

use crate::{
    Builder, HeaderHelper, HttpHelper, Initiator, Middleware, OperateTrait, ProtError, ProtResult,
    RecvRequest, RecvResponse, TimeoutLayer,
};

use super::{WsCodec, Control};


pub struct ServerWsConnection<T> {
    codec: WsCodec<T>,
    inner: InnerConnection,
    timeout: Option<TimeoutLayer>,
}

struct InnerConnection {
    state: State,
    control: Control,
}

#[derive(Debug)]
enum State {
    /// Currently open in a sane state
    Open,

    /// The codec must be flushed
    Closing(Reason, Initiator),

    /// In a closed state
    Closed(Reason, Initiator),
}

unsafe impl<T> Sync for ServerWsConnection<T> {}

unsafe impl<T> Send for ServerWsConnection<T> {}

impl<T> ServerWsConnection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T, builder: Builder) -> ServerWsConnection<T> {
        ServerWsConnection {
            codec: WsCodec::new(io),
            inner: InnerConnection {
                state: State::Open,
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

    // pub fn poll_request(&mut self, cx: &mut Context<'_>) -> Poll<Option<ProtResult<RecvRequest>>> {
    //     if self.timeout.is_some() {
    //         let (ready_time, is_read_end, is_write_end, is_idle) = (
    //             *self.inner.control.get_ready_time(),
    //             self.inner.control.is_read_end(),
    //             self.inner.control.is_write_end(&self.codec),
    //             self.inner.control.is_idle(&self.codec),
    //         );
    //         self.timeout.as_mut().unwrap().poll_ready(
    //             cx,
    //             "server",
    //             ready_time,
    //             is_read_end,
    //             is_write_end,
    //             is_idle,
    //         )?;
    //     }
    //     self.inner.control.poll_request(cx, &mut self.codec)
    //     // loop {
    //     //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
    //     // }
    // }

    // pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<()>> {
    //     self.inner.control.poll_write(cx, &mut self.codec, false)
    // }

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

    // pub async fn incoming<F>(
    //     &mut self,
    //     f: &mut F,
    //     addr: &Option<SocketAddr>,
    //     middles: &mut Vec<Box<dyn Middleware>>,
    // ) -> ProtResult<Option<bool>>
    // where
    //     F: OperateTrait + Send,
    // {
    //     use tokio_stream::StreamExt;
    //     let mut receiver = self.inner.receiver_push.take().unwrap();
    //     tokio::select! {
    //         res = receiver.recv() => {
    //             self.inner.receiver_push = Some(receiver);
    //             if res.is_some() {
    //                 let res = res.unwrap();
    //                 let id = self.inner.control.next_stream_id();
    //                 self.inner.control.send_response_may_push(res.1, res.0, Some(id)).await?;
    //             }
    //         },
    //         req = self.next() => {
    //             self.inner.receiver_push = Some(receiver);
    //             match req {
    //                 None => return Ok(Some(true)),
    //                 Some(Err(e)) => return Err(e),
    //                 Some(Ok(r)) => {
    //                     self.handle_request(addr, r, f, middles).await?;
    //                 }
    //             };
    //         }
    //     }
    //     return Ok(None);
    // }

    // pub fn set_cache_buf(&mut self, read_buf: BinaryMut, write_buf: BinaryMut) {
    //     self.codec.set_cache_buf(read_buf, write_buf)
    // }

    // pub fn set_handshake_status(&mut self, binary: Binary) {
    //     self.inner.control.set_handshake_status(binary, false)
    // }

}

impl<T> Stream for ServerWsConnection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<RecvRequest>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        // loop {
        //     match self.inner.state {
        //         State::Open => {
        //             match self.poll_request(cx) {
        //                 Poll::Pending => {
        //                     return Poll::Pending;
        //                 }
        //                 Poll::Ready(Some(Ok(v))) => {
        //                     return Poll::Ready(Some(Ok(v)));
        //                 }
        //                 Poll::Ready(v) => {
        //                     let _ = self.handle_poll_result(v)?;
        //                     continue;
        //                 }
        //             };
        //         }
        //         State::Closing(reason, initiator) => {
        //             ready!(self.codec.shutdown(cx))?;
        //             self.inner.state = State::Closed(reason, initiator);
        //         }
        //         State::Closed(reason, initiator) => {
        //             if let Err(e) = self.take_error(reason, initiator) {
        //                 return Poll::Ready(Some(Err(e)));
        //             }
        //             return Poll::Ready(None);
        //         }
        //     }
        // }
        Poll::Pending
    }
}
