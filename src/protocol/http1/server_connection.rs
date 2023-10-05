use std::{
    any::{Any, TypeId},
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::{Future, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{Binary, BinaryMut, HeaderName, Request, Response, Serialize};

use crate::{ServerH2Connection, ProtResult, RecvStream};

use super::IoBuffer;

pub struct ServerH1Connection<T> {
    io: IoBuffer<T>,
}

impl<T> ServerH1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        ServerH1Connection {
            io: IoBuffer::new(io),
        }
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
        connect
    }

    pub async fn handle_request<F, Fut, Res, Req>(
        &mut self,
        mut r: Request<RecvStream>,
        f: &mut F,
    ) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>) -> Fut,
        Fut: Future<Output = ProtResult<Option<Response<Res>>>>,
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
                return Err(crate::ProtError::ServerUpgradeHttp2(binary.freeze(), Some(r)));
            }
        }
        let mut content_length = 0;
        if TypeId::of::<Req>() != TypeId::of::<RecvStream>() {
            let _ = r.body_mut().wait_all().await;
            content_length = r.body().body_len();
        }
        match f(r.into_type::<Req>()).await? {
            Some(mut res) => {
                if content_length != 0 && res.get_body_len() == 0 {
                    res.headers_mut()
                        .insert(HeaderName::CONTENT_LENGTH, content_length);
                }
                self.send_response(res.into_type()).await?;
            }
            None => (),
        }
        return Ok(None);
    }

    pub async fn incoming<F, Fut, Res, Req>(&mut self, f: &mut F) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>) -> Fut,
        Fut: Future<Output = ProtResult<Option<Response<Res>>>>,
        Req: From<RecvStream>,
        Req: Serialize + Any,
        RecvStream: From<Res>,
        Res: Serialize + Any,
    {
        use futures_util::stream::StreamExt;
        let req = self.next().await;

        match req {
            None => return Ok(Some(true)),
            Some(Err(e)) => return Err(e),
            Some(Ok(r)) => {
                self.handle_request(r, f).await?;
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
        Pin::new(&mut self.io).poll_request(cx)
    }
}
