use std::{any::{Any, TypeId}, sync::Arc, net::SocketAddr, time::Duration};

use futures_core::{Future};
use tokio::{io::{AsyncRead, AsyncWrite}, sync::Mutex};
use webparse::{http::http2::frame::StreamIdentifier, Binary, Request, Response, Serialize};

use crate::{ServerH2Connection, ProtError, ProtResult, RecvStream, TimeoutLayer};

use super::http1::ServerH1Connection;

pub struct Server<T, D=()>
where
    T: AsyncRead + AsyncWrite + Unpin,
    D: std::marker::Send + 'static
{
    http1: Option<ServerH1Connection<T>>,
    http2: Option<ServerH2Connection<T>>,
    data: Arc<Mutex<D>>,
    addr: Option<SocketAddr>,

    timeout: Option<TimeoutLayer>,
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
    D: std::marker::Send + 'static
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
        self.timeout.as_mut().unwrap().set_read_timeout(read_timeout);
        if let Some(http) = &mut self.http1 {
            http.set_read_timeout(read_timeout);
        }
        if let Some(http) = &mut self.http2 {
            http.set_read_timeout(read_timeout);
        }
    }

    pub fn set_write_timeout(&mut self, write_timeout: Option<Duration>) {
        if self.timeout.is_none() {
            self.timeout = Some(TimeoutLayer::new());
        }
        self.timeout.as_mut().unwrap().set_write_timeout(write_timeout);
        if let Some(http) = &mut self.http1 {
            http.set_write_timeout(write_timeout);
        }
        if let Some(http) = &mut self.http2 {
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
        }
        if let Some(http) = &mut self.http2 {
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
        }
        if let Some(http) = &mut self.http2 {
            http.set_ka_timeout(timeout);
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
    Res: Serialize + Any
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
                    log::trace!("HTTP服务发生错误:{:?}", e);
                    return Err(e)
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
