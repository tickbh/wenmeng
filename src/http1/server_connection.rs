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
// Created Date: 2023/10/07 09:41:02

use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll}, time::Duration,
};

// use futures_core::{Stream};
use tokio::{
    io::{AsyncRead, AsyncWrite},
};
use tokio_stream::{Stream, StreamExt};
use webparse::{Binary, BinaryMut, Response, Serialize, Version};

use crate::{ProtResult, ServerH2Connection, HttpHelper, HeaderHelper, TimeoutLayer, RecvResponse, RecvRequest, OperateTrait, Middleware, ws::ServerWsConnection};

use super::IoBuffer;

pub struct ServerH1Connection<T> {
    io: IoBuffer<T>,

    timeout: Option<TimeoutLayer>,
}

impl<T> ServerH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        ServerH1Connection {
            io: IoBuffer::new(io, true),

            timeout: None,
        }
    }
    
    pub fn new_by_cache(io: T, binary: BinaryMut) -> Self {
        let mut io = IoBuffer::new(io, true);
        io.set_read_cache(binary);
        ServerH1Connection {
            io,
            timeout: None,
        }
    }

    pub fn into_io(self) -> T {
        self.io.into_io()
    }

    pub fn set_read_timeout(&mut self, read_timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout.as_mut().unwrap().set_read_timeout(read_timeout);
    }

    pub fn set_write_timeout(&mut self, write_timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout.as_mut().unwrap().set_write_timeout(write_timeout);
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

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        self.io.poll_write(cx)
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<RecvRequest>>> {
        self.io.poll_request(cx)
    }

    pub fn into_h2(self, binary: Binary) -> ServerH2Connection<T> {
        let (io, read_buf, write_buf) = self.io.into();
        let mut connect = crate::http2::Builder::new().server_connection(io);
        connect.set_cache_buf(read_buf, write_buf);
        connect.set_handshake_status(binary);
        connect.set_timeout_layer(self.timeout);
        connect
    }

    pub fn into_ws(self, binary: Binary) -> ServerWsConnection<T> {
        let (io, read_buf, write_buf) = self.io.into();
        let mut connect = ServerWsConnection::new(io);
        connect.set_cache_buf(read_buf, write_buf);
        connect.set_handshake_status(binary);
        connect.set_timeout_layer(self.timeout);
        connect
    }

    pub async fn handle_request<F>(
        &mut self,
        addr: &Option<SocketAddr>,
        r: RecvRequest,
        f: &mut F,
        middles: &mut Vec<Box<dyn Middleware>>
    ) -> ProtResult<Option<bool>>
    where
        F: OperateTrait + Send,
    {
        
        let mut res = HttpHelper::handle_request(Version::Http11, addr, r, f, middles).await?;
        HeaderHelper::process_response_header(Version::Http11, false, &mut res)?;
        self.send_response(res).await?;
        return Ok(None);
    }

    pub async fn incoming(
        &mut self,
    ) -> ProtResult<Option<RecvRequest>>
    {
        let req = self.next().await;

        match req {
            None => return Ok(None),
            Some(Err(e)) => return Err(e),
            Some(Ok(r)) => {
                return Ok(Some(r));
            }
        };
    }

    pub async fn send_response(&mut self, res: RecvResponse) -> ProtResult<()> {
        self.io.send_response(res)
    }
}

impl<T> Stream for ServerH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<RecvRequest>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.timeout.is_some() {
            let (ready_time, is_read_end, is_write_end, is_idle) = (*self.io.get_ready_time(), self.io.is_read_end(), self.io.is_write_end(), self.io.is_idle());
            self.timeout.as_mut().unwrap().poll_ready(cx, "server", ready_time, is_read_end, is_write_end, is_idle)?;
        }
        Pin::new(&mut self.io).poll_request(cx)
    }
}
