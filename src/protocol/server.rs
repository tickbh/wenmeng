use std::{
    any::{Any, TypeId},
    net::SocketAddr,
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream},
};
use webparse::{http::http2::frame::StreamIdentifier, Binary, Request, Response, Serialize, Version};

use crate::{
    ProtError, ProtResult, RecvRequest, RecvStream, ServerH2Connection, TimeoutLayer, OperateTrait, Middleware,
};

use super::http1::ServerH1Connection;

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

#[derive(Default)]
pub struct ServerOption {
    addr: Option<SocketAddr>,
    timeout: Option<TimeoutLayer>,
    middles: Vec<Box<dyn Middleware>>,
}

pub struct Server<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    http1: Option<ServerH1Connection<T>>,
    http2: Option<ServerH2Connection<T>>,
    middles: Vec<Box<dyn Middleware>>,
    addr: Option<SocketAddr>,

    timeout: Option<TimeoutLayer>,
    req_num: usize,
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
            middles: vec![],
            addr,

            timeout: None,
            req_num: 0,
        }
    }
}

impl<T> Server<T>
where
    T: AsyncRead + AsyncWrite + Unpin
{
    pub fn new_data(io: T, addr: Option<SocketAddr>) -> Self {
        Self {
            http1: Some(ServerH1Connection::new(io)),
            http2: None,
            addr,
            middles: vec![],
            timeout: None,
            req_num: 0,
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

    pub fn get_req_num(&self) -> usize {
        self.req_num
    }

    pub async fn send_response<R>(
        &mut self,
        res: Response<R>,
        stream_id: Option<StreamIdentifier>,
    ) -> ProtResult<()>
    where
        RecvStream: From<R>,
        R: Serialize,
    {
        let _result = if let Some(h1) = &mut self.http1 {
            h1.send_response(res.into_type::<RecvStream>()).await?;
        } else if let Some(h2) = &mut self.http2 {
            if let Some(stream_id) = stream_id {
                let _recv = RecvStream::only(Binary::new());
                let res = res.into_type::<RecvStream>();
                h2.send_response(res, stream_id).await?;
            }
        };

        Ok(())
    }

    pub async fn req_into_type<Req>(mut r: RecvRequest) -> Request<Req>
    where
        Req: From<RecvStream>,
        Req: Serialize + Any,
    {
        if TypeId::of::<Req>() == TypeId::of::<RecvStream>() {
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
        Req: From<RecvStream>,
        Req: Serialize,
    {
        if !r.body().is_end() {
            let _ = r.body_mut().wait_all().await;
        }
        Ok(())
    }

    pub async fn incoming<F>(&mut self, mut f: F) -> ProtResult<Option<bool>>
    where
        F: OperateTrait,
    {
        loop {
            let result = if let Some(h1) = &mut self.http1 {
                h1.incoming(&mut f, &self.addr, &mut self.middles).await
            } else if let Some(h2) = &mut self.http2 {
                h2.incoming(&mut f, &self.addr, &mut self.middles).await
            } else {
                Ok(Some(true))
            };
            match result {
                Ok(None) | Ok(Some(false)) => {
                    self.req_num = self.req_num.wrapping_add(1);
                    continue;
                }
                Err(ProtError::ServerUpgradeHttp2(b, r)) => {
                    if self.http1.is_some() {
                        self.http2 = Some(self.http1.take().unwrap().into_h2(b));
                        if let Some(r) = r {
                            self.http2
                                .as_mut()
                                .unwrap()
                                .handle_request(&self.addr, r, &mut f, &mut self.middles)
                                .await?;

                            self.req_num = self.req_num.wrapping_add(1);
                        }
                        continue;
                    } else {
                        return Err(ProtError::ServerUpgradeHttp2(b, r));
                    }
                }
                Err(e) => {
                    for i in 0usize .. self.middles.len() {
                        self.middles[i].process_error(None, &e).await;
                    }
                    return Err(e);
                }
                Ok(Some(true)) => return Ok(Some(true)),
            };
        }
        // loop {
        //     tokio::select! {
        //         _ = receiver.recv() => {
        //             let _ = poll_fn(|cx| Poll::Ready(self.poll_write(cx))).await;
        //         },
        //         v = self.next() => {
        //             self.inner.receiver = Some(receiver);
        //             return v;
        //         }
        //     }
        // }
    }
}
