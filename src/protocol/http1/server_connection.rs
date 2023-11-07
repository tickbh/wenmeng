use std::{
    any::{Any},
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll}, time::Duration,
};

use futures_core::{Future, Stream};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};
use webparse::{Binary, BinaryMut, Request, Response, Serialize, Version};

use crate::{ProtResult, RecvStream, ServerH2Connection, HttpHelper, HeaderHelper, TimeoutLayer};

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


    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        self.io.poll_write(cx)
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Request<RecvStream>>>> {
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

    pub async fn handle_request<F, Fut, Res, Req>(
        &mut self,
        addr: &Option<SocketAddr>,
        r: Request<RecvStream>,
        f: &mut F,
    ) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>) -> Fut,
        Fut: Future<Output = ProtResult<Response<Res>>>,
        Req: From<RecvStream>,
        Req: Serialize + Any,
        RecvStream: From<Res>,
        Res: Serialize + Any,
    {
        if let Some(protocol) = r.headers().get_upgrade_protocol() {
            if protocol == "h2c" {
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
        }
        let mut res = HttpHelper::handle_request(addr, r, f).await?;
        HeaderHelper::process_response_header(Version::Http11, false, &mut res)?;
        self.send_response(res).await?;
        return Ok(None);
    }

    pub async fn incoming<F, Fut, Res, Req, D>(
        &mut self,
        f: &mut F,
        addr: &Option<SocketAddr>,
        data: &mut Arc<Mutex<D>>,
    ) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>) -> Fut,
        Fut: Future<Output = ProtResult<Response<Res>>>,
        Req: From<RecvStream>,
        Req: Serialize + Any,
        RecvStream: From<Res>,
        Res: Serialize + Any,
        D: std::marker::Send + 'static
    {
        use futures_util::stream::StreamExt;
        let req = self.next().await;

        match req {
            None => return Ok(Some(true)),
            Some(Err(e)) => return Err(e),
            Some(Ok(mut r)) => {
                r.extensions_mut().insert(data.clone());
                self.handle_request(addr, r, f).await?;
            }
        };
        return Ok(None);
    }

    pub async fn send_response(&mut self, res: Response<RecvStream>) -> ProtResult<()> {
        self.io.send_response(res).await
    }
}

impl<T> Stream for ServerH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<Request<RecvStream>>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.timeout.is_some() {
            let (ready_time, is_read_end, is_write_end, is_idle) = (*self.io.get_ready_time(), self.io.is_read_end(), self.io.is_write_end(), self.io.is_idle());
            self.timeout.as_mut().unwrap().poll_ready(cx, ready_time, is_read_end, is_write_end, is_idle)?;
        }
        Pin::new(&mut self.io).poll_request(cx)
    }
}
