use std::{
    any::{Any, TypeId},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use futures_core::Future;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex, net::{TcpListener, TcpStream},
};
use webparse::{http::http2::frame::StreamIdentifier, Binary, Request, Response, Serialize};

use crate::{ProtError, ProtResult, RecvStream, ServerH2Connection, TimeoutLayer};

use super::http1::ServerH1Connection;

#[derive(Debug)]
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

    pub fn stream<T>(self, stream: T) -> Server<T, ()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut server = Server::new(stream, self.inner.addr);
        server.set_timeout_layer(self.inner.timeout.clone());
        server
    }

    pub fn stream_data<T, D>(self, stream: T, data: Arc<Mutex<D>>) -> Server<T, D>
    where
        T: AsyncRead + AsyncWrite + Unpin,
        D: std::marker::Send + 'static,
    {
        let mut server = Server::new_data(stream, self.inner.addr, data);
        server.set_timeout_layer(self.inner.timeout.clone());
        server
    }
}

#[derive(Clone, Debug, Default)]
pub struct ServerOption {
    addr: Option<SocketAddr>,
    timeout: Option<TimeoutLayer>,
}

pub struct Server<T, D = ()>
where
    T: AsyncRead + AsyncWrite + Unpin,
    D: std::marker::Send + 'static,
{
    http1: Option<ServerH1Connection<T>>,
    http2: Option<ServerH2Connection<T>>,
    data: Arc<Mutex<D>>,
    addr: Option<SocketAddr>,

    timeout: Option<TimeoutLayer>,
}

impl Server<TcpStream, ()> {
    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl<T> Server<T, ()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T, addr: Option<SocketAddr>) -> Self {
        Self {
            http1: Some(ServerH1Connection::new(io)),
            http2: None,
            data: Arc::new(Mutex::new(())),
            addr,

            timeout: None,
        }
    }
}

impl<T, D> Server<T, D>
where
    T: AsyncRead + AsyncWrite + Unpin,
    D: std::marker::Send + 'static,
{
    pub fn new_data(io: T, addr: Option<SocketAddr>, data: Arc<Mutex<D>>) -> Self {
        Self {
            http1: Some(ServerH1Connection::new(io)),
            http2: None,
            data,
            addr,
            timeout: None,
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

    pub async fn req_into_type<Req>(mut r: Request<RecvStream>) -> Request<Req>
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

    pub async fn try_wait_req<Req>(r: &mut Request<RecvStream>) -> ProtResult<()>
    where
        Req: From<RecvStream>,
        Req: Serialize,
    {
        if !r.body().is_end() {
            let _ = r.body_mut().wait_all().await;
        }
        Ok(())
    }

    pub async fn incoming<F, Fut, Res, Req>(&mut self, mut f: F) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>) -> Fut,
        Fut: Future<Output = ProtResult<Response<Res>>>,
        Req: From<RecvStream>,
        Req: Serialize + Any,
        RecvStream: From<Res>,
        Res: Serialize + Any,
    {
        loop {
            let result = if let Some(h1) = &mut self.http1 {
                h1.incoming(&mut f, &self.addr, &mut self.data).await
            } else if let Some(h2) = &mut self.http2 {
                h2.incoming(&mut f, &self.addr, &mut self.data).await
            } else {
                Ok(Some(true))
            };
            match result {
                Ok(None) | Ok(Some(false)) => continue,
                Err(ProtError::ServerUpgradeHttp2(b, r)) => {
                    if self.http1.is_some() {
                        self.http2 = Some(self.http1.take().unwrap().into_h2(b));
                        if let Some(mut r) = r {
                            r.extensions_mut().insert(self.data.clone());
                            self.http2
                                .as_mut()
                                .unwrap()
                                .handle_request(&self.addr, r, &mut f)
                                .await?;
                        }
                        continue;
                    } else {
                        return Err(ProtError::ServerUpgradeHttp2(b, r));
                    }
                }
                Err(e) => {
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
