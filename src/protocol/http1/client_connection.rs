use std::{
    pin::Pin,
    task::{Context, Poll}, time::{Duration, Instant},
};

use futures_core::{Stream, Future};
use tokio::{io::{AsyncRead, AsyncWrite}};
use webparse::{Binary, Request, Response, http2::{HTTP2_MAGIC, frame::Settings}};

use crate::{ProtResult, RecvStream, http2::ClientH2Connection};

use super::IoBuffer;

pub struct ClientH1Connection<T> {
    io: IoBuffer<T>,
    settings: Option<Settings>,

    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    timeout: Option<Duration>,
}

impl<T> ClientH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        ClientH1Connection {
            io: IoBuffer::new(io, false),
            settings: None,

            read_timeout: None,
            write_timeout: None,
            timeout: None,
        }
    }

    pub fn set_read_timeout(&mut self, read_timeout: Option<Duration>) {
        self.read_timeout = read_timeout;
    }

    pub fn set_write_timeout(&mut self, write_timeout: Option<Duration>) {
        self.write_timeout = write_timeout;
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
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

    pub fn into_h2(self, settings: Settings) -> ClientH2Connection<T> {
        let (io, read_buf, write_buf) = self.io.into();
        let mut connect = crate::http2::Builder::new().client_connection(io);
        connect.set_cache_buf(read_buf, write_buf);
        connect.set_handshake_status(Binary::from_static(HTTP2_MAGIC));
        connect.set_setting_status(settings, false);
        connect.next_stream_id();
        connect
    }

    pub async fn handle_response(
        &mut self,
        r: Response<RecvStream>,
    ) -> ProtResult<Option<Response<RecvStream>>>
    {
        if r.status() == 101 {
            return Err(crate::ProtError::ClientUpgradeHttp2(self.settings.clone().unwrap_or(Settings::default())))
        }
        return Ok(Some(r));
    }

    pub async fn incoming(&mut self) -> ProtResult<Option<Response<RecvStream>>>
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

    pub async fn send_response(&mut self, res: Response<RecvStream>) -> ProtResult<()> {
        self.io.send_response(res).await
    }

    pub fn send_request(&mut self, mut req: Request<RecvStream>) -> ProtResult<()> {
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
    type Item = ProtResult<Response<RecvStream>>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let now = Instant::now();
        if !self.io.is_read_end() {
            if let Some(read) = &self.read_timeout {
                let next = *self.io.get_ready_time() + *read;
                if now > next {
                    return Poll::Ready(Some(Err(crate::ProtError::Extension("read timeout"))));
                }
                
                let _ = Pin::new(&mut Box::pin(tokio::time::sleep_until(next.into()))).poll(cx);
            }
        }
        if !self.io.is_write_end() {
            if let Some(write) = &self.write_timeout {
                let next = *self.io.get_ready_time() + *write;
                if now > next {
                    return Poll::Ready(Some(Err(crate::ProtError::Extension("write timeout"))));
                }
                let _ = Pin::new(&mut Box::pin(tokio::time::sleep_until(next.into()))).poll(cx);

            }
        }

        if !self.io.is_end() {
            if let Some(time) = &self.timeout {
                let next = *self.io.get_ready_time() + *time;
                if now > next {
                    return Poll::Ready(Some(Err(crate::ProtError::Extension("timeout"))));
                }
                let _ = Pin::new(&mut Box::pin(tokio::time::sleep_until(next.into()))).poll(cx);
            }
        }
        Pin::new(&mut self.io).poll_response(cx)
    }
}
