use std::{
    any::{Any, TypeId},
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures_core::{Future, Stream};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};
use webparse::{Binary, BinaryMut, HeaderName, Request, Response, Serialize};

use crate::{ProtResult, RecvStream, ServerH2Connection};

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

    pub async fn handle_request<F, Fut, Res, Req, D>(
        &mut self,
        addr: &Option<SocketAddr>,
        data: &mut Arc<Mutex<D>>,
        mut r: Request<RecvStream>,
        f: &mut F,
    ) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>, Arc<Mutex<D>>) -> Fut,
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
                return Err(crate::ProtError::ServerUpgradeHttp2(
                    binary.freeze(),
                    Some(r),
                ));
            }
        }
        if TypeId::of::<Req>() != TypeId::of::<RecvStream>() {
            let _ = r.body_mut().wait_all().await;
        }
        if let Some(addr) = addr {
            r.headers_mut().system_insert("$client_ip".to_string(), format!("{}", addr));
        }
        match f(r.into_type::<Req>(), data.clone()).await? {
            Some(res) => {
                let mut res = res.into_type::<RecvStream>();
                if res.get_body_len() == 0 && res.body().is_end() {
                    let len = res.body().body_len();
                    res.headers_mut().insert(HeaderName::CONTENT_LENGTH, len);
                }
                self.send_response(res).await?;
            }
            None => (),
        }
        return Ok(None);
    }

    pub async fn incoming<F, Fut, Res, Req, D>(
        &mut self,
        f: &mut F,
        addr: &Option<SocketAddr>,
        data: &mut Arc<Mutex<D>>,
    ) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>, Arc<Mutex<D>>) -> Fut,
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
                self.handle_request(addr, data, r, f).await?;
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
