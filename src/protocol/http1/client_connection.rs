use std::{
    any::{Any, TypeId},
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::{Future, Stream};
use tokio::{io::{AsyncRead, AsyncWrite}, sync::mpsc::Sender};
use webparse::{Binary, BinaryMut, HeaderName, Request, Response, Serialize};

use crate::{ServerH2Connection, ProtResult, RecvStream, http2::ClientH2Connection};

use super::IoBuffer;

pub struct ClientH1Connection<T> {
    io: IoBuffer<T>,
}

impl<T> ClientH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        ClientH1Connection {
            io: IoBuffer::new(io),
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<()>> {
        self.io.poll_write(cx)
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Request<RecvStream>>>> {
        self.io.poll_request(cx)
    }

    pub fn into_h2(self) -> ClientH2Connection<T> {
        let (io, read_buf, write_buf) = self.io.into();
        let mut connect = crate::http2::Builder::new().client_connection(io);
        connect.set_cache_buf(read_buf, write_buf);
        connect.set_handshake_status(Binary::new());
        connect
    }

    pub async fn handle_request(
        &mut self,
        r: Response<RecvStream>,
    ) -> ProtResult<Option<bool>>
    {
        if r.status() == 101 {
            return Err(crate::ProtError::ClientUpgradeHttp2)
        }
        return Ok(None);
    }

    pub async fn incoming(&mut self) -> ProtResult<Option<Response<RecvStream>>>
    {
        use futures_util::stream::StreamExt;
        let req = self.next().await;

        match req {
            None => return Ok(None),
            Some(Err(e)) => return Err(e),
            Some(Ok(r)) => {
                return Ok(Some(r))
            }
        };
        return Ok(None);
    }

    pub async fn send_response(&mut self, res: Response<RecvStream>) -> ProtResult<()> {
        self.io.send_response(res).await
    }

    pub async fn send_request(&mut self, req: Request<RecvStream>) -> ProtResult<()> {
        self.io.send_request(req).await
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
        Pin::new(&mut self.io).poll_response(cx)
    }
}
