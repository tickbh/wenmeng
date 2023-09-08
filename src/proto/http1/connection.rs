use std::{task::{Context, Poll}, future::poll_fn};

use futures_core::Stream;
use tokio::{io::{AsyncRead, AsyncWrite}, sync::mpsc::Receiver};
use webparse::Request;

use crate::{ProtoResult, RecvStream};

use super::IoBuffer;

pub struct H1Connection<T> {
    io: IoBuffer<T>,
    // codec: Codec<T>,
    // inner: InnerConnection,

    
    /// 通知有新内容要求要写入
    receiver: Option<Receiver<()>>,
}

impl<T> H1Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        H1Connection {
            io: IoBuffer::new(io),
            receiver: None,
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtoResult<()>> {
        println!("poll write!!!!!!!");
        Poll::Pending
        // self.inner.control.poll_write(cx, &mut self.codec)
        // loop {
        //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
        // }
    }

    
    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtoResult<Request<RecvStream>>>> {
        self.io.poll_request(cx)
    }

    
    pub async fn incoming(&mut self) -> Option<ProtoResult<Request<RecvStream>>> {
        use futures_util::stream::StreamExt;
        let mut receiver = self.receiver.take().unwrap();
        loop {
            tokio::select! {
                _ = receiver.recv() => {
                    let _ = poll_fn(|cx| Poll::Ready(self.poll_write(cx))).await;
                },
                v = self.next() => {
                    self.receiver = Some(receiver);
                    return v;
                }
            }
        }
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
        Poll::Pending
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
