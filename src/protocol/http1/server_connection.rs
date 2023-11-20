use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll}, time::Duration,
};

use futures_core::{Stream};
use tokio::{
    io::{AsyncRead, AsyncWrite},
};
use webparse::{Binary, BinaryMut, Response, Serialize, Version};

use crate::{ProtResult, ServerH2Connection, HttpHelper, HeaderHelper, TimeoutLayer, RecvResponse, RecvRequest, OperateTrait, Middleware};

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

    pub async fn handle_request<F>(
        &mut self,
        addr: &Option<SocketAddr>,
        r: RecvRequest,
        f: &mut F,
        middles: &mut Vec<Box<dyn Middleware>>
    ) -> ProtResult<Option<bool>>
    where
        F: OperateTrait,
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
        let mut res = HttpHelper::handle_request(addr, r, f, middles).await?;
        HeaderHelper::process_response_header(Version::Http11, false, &mut res)?;
        self.send_response(res).await?;
        return Ok(None);
    }

    pub async fn incoming<F>(
        &mut self,
        f: &mut F,
        addr: &Option<SocketAddr>,
        middles: &mut Vec<Box<dyn Middleware>>,
    ) -> ProtResult<Option<bool>>
    where
        F: OperateTrait,
    {
        use futures_util::stream::StreamExt;
        let req = self.next().await;

        match req {
            None => return Ok(Some(true)),
            Some(Err(e)) => return Err(e),
            Some(Ok(r)) => {
                self.handle_request(addr, r, f, middles).await?;
            }
        };
        return Ok(None);
    }

    pub async fn send_response(&mut self, res: RecvResponse) -> ProtResult<()> {
        self.io.send_response(res).await
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
