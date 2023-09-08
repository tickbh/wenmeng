use tokio::io::{AsyncRead, AsyncWrite};
use webparse::Request;

use crate::{H2Connection, ProtoResult, RecvStream};

use super::http1::H1Connection;

pub struct Server<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    http1: Option<H1Connection<T>>,
    http2: Option<H2Connection<T>>,
}

impl<T> Server<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        Self {
            http1: Some(H1Connection::new(io)),
            http2: None,
        }
    }

    
    pub async fn incoming(&mut self) -> Option<ProtoResult<Request<RecvStream>>> {
        use futures_util::stream::StreamExt;
        // loop {
        //     tokio::select! {
        //         _ = receiver.recv() => {
        //             let _ = poll_fn(|cx| Poll::Ready(self.poll_write(cx))).await;
        //         },
        //         v = self.next() => {
        //             self.inner.receiver = Some(receiver);
        //             return v;
        //         }
        //     }
        // }
        None
    }
}
