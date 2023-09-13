use std::{
    any::{Any, TypeId},
    future::poll_fn,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::{Future, Stream};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Receiver,
};
use webparse::{http::response, Binary, BinaryMut, Request, Response, Serialize};

use crate::{protocol::recv_stream, H2Connection, ProtResult, RecvStream};

use super::IoBuffer;

pub struct H1Connection<T> {
    io: IoBuffer<T>,
    // codec: Codec<T>,
}

impl<T> H1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        H1Connection {
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

    pub fn into_h2(self, binary: Binary) -> H2Connection<T> {
        let (io, read_buf, write_buf) = self.io.into();
        let mut connect = crate::http2::Builder::new().connection(io);
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
                return Err(crate::ProtError::UpgradeHttp2(binary.freeze(), Some(r)));
            }
        }
        if TypeId::of::<Req>() != TypeId::of::<RecvStream>() {
            let _ = r.body_mut().wait_all().await;
        }
        match f(r.into_type::<Req>()).await? {
            Some(res) => {
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

impl<T> Stream for H1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<Request<RecvStream>>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.io).poll_request(cx)
        // loop {
        //     match self.inner.state {
        //         State::Open => {
        //             match self.poll_request(cx) {
        //                 Poll::Pending => {
        //                     println!("pending");
        //                     // if self.inner.control.error.is_some() {
        //                     //     self.inner.control.go_away_now(Reason::NO_ERROR);
        //                     //     continue;
        //                     // }
        //                     return Poll::Pending;
        //                 }
        //                 Poll::Ready(Some(Ok(v))) => {
        //                     return Poll::Ready(Some(Ok(v)));
        //                 }
        //                 Poll::Ready(v) => {
        //                     let _ = self.handle_poll_result(v)?;
        //                     continue;
        //                 }
        //             };
        //         },
        //         State::Closing(reason, initiator) => {
        //             ready!(self.codec.shutdown(cx))?;

        //             // Transition the state to error
        //             self.inner.state = State::Closed(reason, initiator);
        //         },
        //         State::Closed(reason, initiator) =>  {
        //             if let Err(e) = self.take_error(reason, initiator) {
        //                 return Poll::Ready(Some(Err(e)));
        //             }
        //             return Poll::Ready(None);
        //         },
        //     }

        // }
        // let xxx = self.poll_request(cx);
        // println!("connect === {:?} ", xxx.is_pending());
        // xxx
    }
}
