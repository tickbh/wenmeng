use std::{
    future::poll_fn,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::{Stream, Future};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Receiver,
};
use webparse::{Request, Response, Serialize, BinaryMut};

use crate::{H2Connection, ProtoResult, RecvStream};

use super::IoBuffer;

pub struct H1Connection<T> {
    io: IoBuffer<T>,
    // codec: Codec<T>,

    /// 通知有新内容要求要写入
    receiver: Option<Receiver<()>>,
}

impl<T> H1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<()>(1);
        H1Connection {
            io: IoBuffer::new(io, sender),
            receiver: Some(receiver),
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtoResult<()>> {
        self.io.poll_write(cx)
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtoResult<Request<RecvStream>>>> {
        self.io.poll_request(cx)
    }

    pub fn into_h2(self) -> H2Connection<T> {
        let (io, read_buf, write_buf) = self.io.into();
        let mut connect = crate::http2::Builder::new().connection(io);
        connect.set_cache_buf(read_buf, write_buf);
        connect.set_handshake_ok();
        connect
    }

    pub async fn incoming<F, Fut, R>(&mut self, f: &mut F) -> ProtoResult<Option<bool>>
    where
    F: FnMut(Request<RecvStream>) -> Fut,
    Fut: Future<Output = ProtoResult<Option<Response<R>>>>,
    RecvStream: From<R>,
    R: Serialize {
        use futures_util::stream::StreamExt;
        let req = self.next().await;

        match req {
            None => return Ok(Some(true)),
            Some(Err(e)) => return Err(e),
            Some(Ok(r)) => {
                match f(r).await? {
                    Some(res) => {
                        self.send_response(res.into_type()).await?;
                    }
                    None => (),
                }
            }
        };
        return Ok(None)
    }

    pub async fn send_response(&mut self, res: Response<RecvStream>) -> ProtoResult<()>
    {
        self.io.send_response(res).await
    }
}

impl<T> Stream for H1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtoResult<Request<RecvStream>>;
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
