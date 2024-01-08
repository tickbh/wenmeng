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

use std::{
    any::{Any, TypeId},
    future::poll_fn,
    net::SocketAddr,
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream, sync::mpsc::{channel, Receiver},
};
use tokio_stream::StreamExt;
use webparse::{
    http::http2::frame::StreamIdentifier, Binary, BinaryMut, Request, Response, Serialize, ws::OwnedMessage,
};

use super::{http1::ServerH1Connection, middle::BaseMiddleware};
use crate::{
    ws::{ServerWsConnection, WsHandshake, WsTrait, WsOption},
    Body, HttpTrait, Middleware, ProtError, ProtResult, RecvRequest, ServerH2Connection,
    TimeoutLayer,
};

pub struct Builder {
    inner: ServerOption,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            inner: ServerOption::default(),
        }
    }

    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        if self.inner.timeout.is_none() {
            self.inner.timeout = Some(TimeoutLayer::new());
        }
        self.inner.timeout.as_mut().unwrap().connect_timeout = Some(connect_timeout);
        self
    }

    pub fn ka_timeout(mut self, ka_timeout: Duration) -> Self {
        if self.inner.timeout.is_none() {
            self.inner.timeout = Some(TimeoutLayer::new());
        }
        self.inner.timeout.as_mut().unwrap().ka_timeout = Some(ka_timeout);
        self
    }

    pub fn read_timeout(mut self, read_timeout: Duration) -> Self {
        if self.inner.timeout.is_none() {
            self.inner.timeout = Some(TimeoutLayer::new());
        }
        self.inner.timeout.as_mut().unwrap().read_timeout = Some(read_timeout);
        self
    }

    pub fn write_timeout(mut self, write_timeout: Duration) -> Self {
        if self.inner.timeout.is_none() {
            self.inner.timeout = Some(TimeoutLayer::new());
        }
        self.inner.timeout.as_mut().unwrap().write_timeout = Some(write_timeout);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        if self.inner.timeout.is_none() {
            self.inner.timeout = Some(TimeoutLayer::new());
        }
        self.inner.timeout.as_mut().unwrap().timeout = Some(timeout);
        self
    }

    pub fn timeout_layer(mut self, timeout: Option<TimeoutLayer>) -> Self {
        self.inner.timeout = timeout;
        self
    }

    pub fn addr(mut self, addr: SocketAddr) -> Self {
        self.inner.addr = Some(addr);
        self
    }

    pub fn value(self) -> ServerOption {
        self.inner
    }

    pub fn middle<M: Middleware + 'static>(mut self, middle: M) -> Self {
        self.inner.middles.push(Box::new(middle));
        self
    }

    pub fn stream<T>(self, stream: T) -> Server<T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut server = Server::new(stream, self.inner.addr);
        server.set_timeout_layer(self.inner.timeout.clone());
        server
    }
}

// #[derive(Default)]
pub struct ServerOption {
    addr: Option<SocketAddr>,
    timeout: Option<TimeoutLayer>,
    middles: Vec<Box<dyn Middleware>>,
}

impl Default for ServerOption {
    fn default() -> Self {
        Self {
            addr: Default::default(),
            timeout: Default::default(),
            middles: vec![Box::new(BaseMiddleware::new(false))],
        }
    }
}

pub struct Server<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Sized,
{
    http1: Option<ServerH1Connection<T>>,
    http2: Option<ServerH2Connection<T>>,
    ws: Option<ServerWsConnection<T>>,
    middles: Vec<Box<dyn Middleware>>,
    callback_http: Option<Box<dyn HttpTrait>>,
    callback_ws: Option<Box<dyn WsTrait>>,
    addr: Option<SocketAddr>,
    timeout: Option<TimeoutLayer>,
    req_num: usize,
    max_req_num: usize,
}

impl Server<TcpStream> {
    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl<T> Server<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T, addr: Option<SocketAddr>) -> Self {
        Self {
            http1: Some(ServerH1Connection::new(io)),
            http2: None,
            ws: None,
            middles: vec![],
            addr,
            callback_http: None,
            callback_ws: None,

            timeout: None,
            req_num: 0,
            max_req_num: usize::MAX,
        }
    }
}

impl<T> Server<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new_by_cache(io: T, addr: Option<SocketAddr>, binary: BinaryMut) -> Self {
        Self {
            http1: Some(ServerH1Connection::new_by_cache(io, binary)),
            http2: None,
            ws: None,
            addr,
            middles: vec![],
            callback_http: None,
            callback_ws: None,
            timeout: None,
            req_num: 0,
            max_req_num: usize::MAX,
        }
    }

    pub fn into_io(self) -> T {
        if self.http1.is_some() {
            self.http1.unwrap().into_io()
        } else {
            self.http2.unwrap().into_io()
        }
    }

    pub fn set_read_timeout(&mut self, read_timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout
            .as_mut()
            .unwrap()
            .set_read_timeout(read_timeout);
        if let Some(http) = &mut self.http1 {
            http.set_read_timeout(read_timeout);
        } else if let Some(http) = &mut self.http2 {
            http.set_read_timeout(read_timeout);
        }
    }

    pub fn set_write_timeout(&mut self, write_timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout
            .as_mut()
            .unwrap()
            .set_write_timeout(write_timeout);
        if let Some(http) = &mut self.http1 {
            http.set_write_timeout(write_timeout);
        } else if let Some(http) = &mut self.http2 {
            http.set_write_timeout(write_timeout);
        }
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout.as_mut().unwrap().set_timeout(timeout);
        if let Some(http) = &mut self.http1 {
            http.set_timeout(timeout);
        } else if let Some(http) = &mut self.http2 {
            http.set_timeout(timeout);
        }
    }

    pub fn set_ka_timeout(&mut self, timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout.as_mut().unwrap().set_ka_timeout(timeout);
        if let Some(http) = &mut self.http1 {
            http.set_ka_timeout(timeout);
        } else if let Some(http) = &mut self.http2 {
            http.set_ka_timeout(timeout);
        }
    }

    pub fn set_timeout_layer(&mut self, timeout: Option<TimeoutLayer>) {
        self.timeout = timeout.clone();
        if let Some(http) = &mut self.http1 {
            http.set_timeout_layer(timeout);
        } else if let Some(http) = &mut self.http2 {
            http.set_timeout_layer(timeout);
        }
    }

    pub fn middle<M: Middleware + 'static>(&mut self, middle: M) {
        self.middles.push(Box::new(middle));
    }

    pub fn set_callback_http(&mut self, callback_http: Box<dyn HttpTrait>) {
        self.callback_http = Some(callback_http);
    }

    pub fn take_callback_http(&mut self) -> Option<Box<dyn HttpTrait>> {
        self.callback_http.take()
    }

    pub fn set_callback_ws(&mut self, callback_ws: Box<dyn WsTrait>) {
        self.callback_ws = Some(callback_ws);
    }

    pub fn take_callback_ws(&mut self) -> Option<Box<dyn WsTrait>> {
        self.callback_ws.take()
    }

    pub fn get_req_num(&self) -> usize {
        self.req_num
    }

    pub async fn send_response<R>(
        &mut self,
        res: Response<R>,
        stream_id: Option<StreamIdentifier>,
    ) -> ProtResult<()>
    where
        Body: From<R>,
        R: Serialize,
    {
        let _result = if let Some(h1) = &mut self.http1 {
            h1.send_response(res.into_type::<Body>()).await?;
        } else if let Some(h2) = &mut self.http2 {
            if let Some(stream_id) = stream_id {
                let _recv = Body::only(Binary::new());
                let res = res.into_type::<Body>();
                h2.send_response(res, stream_id).await?;
            }
        };

        Ok(())
    }

    pub async fn req_into_type<Req>(mut r: RecvRequest) -> Request<Req>
    where
        Req: From<Body>,
        Req: Serialize + Any,
    {
        if TypeId::of::<Req>() == TypeId::of::<Body>() {
            r.into_type::<Req>()
        } else {
            if !r.body().is_end() {
                let _ = r.body_mut().wait_all().await;
            }
            r.into_type::<Req>()
        }
    }

    pub async fn try_wait_req<Req>(r: &mut RecvRequest) -> ProtResult<()>
    where
        Req: From<Body>,
        Req: Serialize,
    {
        if !r.body().is_end() {
            let _ = r.body_mut().wait_all().await;
        }
        Ok(())
    }

    pub async fn handle_request(&mut self, r: RecvRequest) -> ProtResult<Option<bool>>
    {
        if self.callback_http.is_none() {
            return Err(ProtError::Extension("http callback is none"));
        }
        let result = if let Some(h1) = &mut self.http1 {
            h1.handle_request(&self.addr, r, self.callback_http.as_mut().unwrap(), &mut self.middles).await
        } else if let Some(h2) = &mut self.http2 {
            h2.handle_request(&self.addr, r, self.callback_http.as_mut().unwrap(), &mut self.middles).await
        } else {
            Ok(None)
        };
        return result;
    }

    async fn handle_error(&mut self, err: crate::ProtError) -> ProtResult<()>
    {
        if self.callback_http.is_none() {
            return Err(err);
        }
        match err {
            ProtError::ServerUpgradeHttp2(b, r) => {
                if self.http1.is_some() {
                    self.http2 = Some(self.http1.take().unwrap().into_h2(b));
                    if let Some(r) = r {
                        self.http2
                            .as_mut()
                            .unwrap()
                            .handle_request(&self.addr, r, self.callback_http.as_mut().unwrap(), &mut self.middles)
                            .await?;

                        self.req_num = self.req_num.wrapping_add(1);
                    }
                    Ok(())
                } else {
                    return Err(ProtError::ServerUpgradeHttp2(b, r));
                }
            }
            _ => {
                for i in 0usize..self.middles.len() {
                    self.middles[i].process_error(None, &err).await;
                }
                return Err(err);
            }
        }
    }

    pub async fn inner_incoming(&mut self) -> ProtResult<Option<RecvRequest>> {
        let result = if let Some(h1) = &mut self.http1 {
            h1.incoming().await
        } else if let Some(h2) = &mut self.http2 {
            h2.incoming().await
        } else {
            Ok(None)
        };

        match result? {
            None => {
                self.flush().await?;
                return Ok(None);
            }

            Some(r) => {
                if let Some(protocol) = r.headers().get_upgrade_protocol() {
                    match &*protocol {
                        "h2c" => {
                            let mut response = Response::builder()
                                .status(101)
                                .header("Connection", "Upgrade")
                                .header("Upgrade", "h2c")
                                .body(())
                                .unwrap();
                            let mut binary = BinaryMut::new();
                            let _ = response.serialize(&mut binary);
                            return Err(crate::ProtError::ServerUpgradeHttp2(
                                binary.freeze(),
                                Some(r),
                            ));
                        }
                        "websocket" => {
                            return Err(crate::ProtError::ServerUpgradeWs(r));
                        }
                        _ => {}
                    }
                }
                self.req_num = self.req_num.wrapping_add(1);
                return Ok(Some(r));
            }
        };
    }

    pub async fn incoming(&mut self) -> ProtResult<()>
    {
        let (mut ws_receiver, mut ws_option);
        loop {
            match self.inner_incoming().await {
                Err(ProtError::ServerUpgradeWs(r)) => {
                    if self.callback_ws.is_none() {
                        return Err(ProtError::Extension("websocket callback is none"));
                    }
                    let mut response = self.callback_ws.as_mut().unwrap().on_request(&r)?;
                    if response.status() != 101 {
                        self.send_response(response, None).await?;
                        self.flush().await?;
                        return Ok(());
                    }
                    let mut binary = BinaryMut::new();
                    let _ = response.serialize(&mut binary);
                    let (sender, receiver) = channel::<OwnedMessage>(10);
                    let shake = WsHandshake::new(sender, Some(r), response, self.addr.clone());
                    ws_option = self.callback_ws.as_mut().unwrap().on_open(shake)?;
    
                    let value = if let Some(h1) = self.http1.take() {
                        h1.into_ws(binary.freeze())
                    } else if let Some(h2) = self.http2.take() {
                        h2.into_ws(binary.freeze())
                    } else {
                        return Err(ProtError::Extension("unknow version"));
                    };
                    self.ws = Some(value);
                    ws_receiver = receiver;
                    if ws_option.is_some() && ws_option.as_mut().unwrap().receiver.is_some() {
                        ws_receiver = ws_option.as_mut().unwrap().receiver.take().unwrap();
                    }
                    break;
                }
                Err(e) => {
                    self.handle_error(e).await?;
                }
                Ok(None) => return Ok(()),
                Ok(Some(r)) => {
                    self.handle_request(r).await?;
                }
            }

            if self.req_num >= self.max_req_num || (self.callback_http.is_some() && !self.callback_http.as_mut().unwrap().is_continue_next()) {
                self.flush().await?;
                return Ok(());
            }
        }

        if let Err(e) = self.inner_oper_ws(ws_receiver, ws_option).await {
            self.callback_ws.as_mut().unwrap().on_error(e).await;
        }

        Ok(())
    }

    async fn inner_oper_ws(&mut self, mut receiver: Receiver<OwnedMessage>, mut option: Option<WsOption>) -> ProtResult<()>
    {
        if self.callback_ws.is_none() {
            return Err(ProtError::Extension("websocket callback is none"));
        }
        loop {
            if let Some(ws) = &mut self.ws {
                tokio::select! {
                    ret = ws.next() => {
                        match ret {
                            None => {
                                return Ok(());
                            }
                            Some(Ok(msg)) => {
                                match msg {
                                    OwnedMessage::Text(_) | OwnedMessage::Binary(_) => self.callback_ws.as_mut().unwrap().on_message(msg).await?,
                                    OwnedMessage::Close(c) => {
                                        self.callback_ws.as_mut().unwrap().on_close(&c).await;
                                        ws.receiver_close(c)?;
                                    },
                                    OwnedMessage::Ping(v) => {
                                        let p = self.callback_ws.as_mut().unwrap().on_ping(v).await?;
                                        ws.send_owned_message(p)?;
                                    },
                                    OwnedMessage::Pong(v) => {
                                        self.callback_ws.as_mut().unwrap().on_pong(v).await;
                                    },
                                }
                            }
                            Some(Err(e)) => return Err(e),
                        }
                    }
                    msg = receiver.recv() => {
                        match msg {
                            None => {
                                return Ok(());
                            }
                            Some(msg) => {
                                match &msg {
                                    OwnedMessage::Close(data) => {
                                        ws.receiver_close(data.clone())?;
                                    },
                                    _ => {}
                                }
                                ws.send_owned_message(msg)?;
                            }
                        }
                    }
                    _ = WsOption::interval_wait(&mut option) => {
                        self.callback_ws.as_mut().unwrap().on_interval(&mut option).await?;
                    }
                }
            }
        }
    }

    pub async fn flush(&mut self) -> ProtResult<()> {
        if let Some(h1) = &mut self.http1 {
            let _ = poll_fn(|cx| h1.poll_write(cx)).await;
        } else if let Some(h2) = &mut self.http2 {
            let _ = poll_fn(|cx| h2.poll_write(cx)).await;
        };
        return Ok(());
    }

    pub fn set_max_req(&mut self, num: usize) {
        self.max_req_num = num;
    }
}
