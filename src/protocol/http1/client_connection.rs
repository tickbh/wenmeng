use std::{
    pin::Pin,
    task::{Context, Poll}, time::{Duration, Instant},
};

use futures_core::{Stream, Future};
use tokio::{io::{AsyncRead, AsyncWrite}};
use webparse::{Binary, Request, Response, http2::{HTTP2_MAGIC, frame::Settings}};

use crate::{ProtResult, RecvStream, http2::ClientH2Connection, TimeoutLayer, RecvResponse, RecvRequest};

use super::IoBuffer;

pub struct ClientH1Connection<T> {
    io: IoBuffer<T>,
    settings: Option<Settings>,
    timeout: Option<TimeoutLayer>,
}

impl<T> ClientH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        ClientH1Connection {
            io: IoBuffer::new(io, false),
            settings: None,

            timeout: None,
        }
    }

    pub fn set_timeout_layer(&mut self, timeout_layer: Option<TimeoutLayer>) {
        self.timeout = timeout_layer;
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
    ) -> Poll<Option<ProtResult<RecvRequest>>> {
        self.io.poll_request(cx)
    }

    pub fn into_h2(self, settings: Settings) -> ClientH2Connection<T> {
        let (io, read_buf, write_buf) = self.io.into();
        let mut connect = crate::http2::Builder::new().client_connection(io);
        connect.set_cache_buf(read_buf, write_buf);
        connect.set_handshake_status(Binary::from_static(HTTP2_MAGIC));
        connect.set_setting_status(settings, false);
        connect.next_stream_id();
        connect.set_timeout_layer(self.timeout);
        connect
    }

    pub async fn handle_response(
        &mut self,
        r: RecvResponse,
    ) -> ProtResult<Option<RecvResponse>>
    {
        if r.status() == 101 {
            return Err(crate::ProtError::ClientUpgradeHttp2(self.settings.clone().unwrap_or(Settings::default())))
        }
        return Ok(Some(r));
    }

    pub async fn incoming(&mut self) -> ProtResult<Option<RecvResponse>>
    {
        use futures_util::stream::StreamExt;
        let req = self.next().await;

        match req {
            None => return Ok(None),
            Some(Err(e)) => return Err(e),
            Some(Ok(r)) => {
                return self.handle_response(r).await;
            }
        };
    }

    pub async fn send_response(&mut self, res: RecvResponse) -> ProtResult<()> {
        self.io.send_response(res).await
    }

    pub fn send_request(&mut self, mut req: RecvRequest) -> ProtResult<()> {
        if let Some(s) = req.extensions_mut().remove::<Settings>() {
            self.settings = Some(s);
        }
        self.io.send_request(req)
    }
}

impl<T> Stream for ClientH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<RecvResponse>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.timeout.is_some() {
            let (ready_time, is_read_end, is_write_end, is_idle) = (*self.io.get_ready_time(), self.io.is_read_end(), self.io.is_write_end(), self.io.is_idle());
            self.timeout.as_mut().unwrap().poll_ready(cx, "client", ready_time, is_read_end, is_write_end, is_idle)?;
        }
        Pin::new(&mut self.io).poll_response(cx)
    }
}
