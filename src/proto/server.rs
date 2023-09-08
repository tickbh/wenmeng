use futures_core::Future;
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{Binary, Request, Response, Serialize};

use crate::{H2Connection, ProtoError, ProtoResult, RecvStream, SendStream, SendControl};

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

    pub async fn send_response<R: Serialize>(&mut self, res: Response<R>) -> ProtoResult<()> {
        let result = if let Some(h1) = &mut self.http1 {
            h1.send_response(res).await?;
        } else if let Some(h2) = &mut self.http2 {
        };

        Ok(())
    }

    pub async fn incoming<F, Fut>(&mut self, mut f: F) -> ProtoResult<Option<()>>
    where
        F: FnMut(Request<RecvStream>) -> Fut,
        Fut: Future<Output = ProtoResult<Option<Response<Binary>>>>,
    {
        use futures_util::stream::StreamExt;
        loop {
            let result = if let Some(h1) = &mut self.http1 {
                h1.incoming().await
            } else if let Some(h2) = &mut self.http2 {
                h2.incoming().await
            } else {
                None
            };
            match result {
                None => return Ok(None),
                Some(Err(ProtoError::UpgradeHttp2)) => {
                    if self.http1.is_some() {
                        self.http2 = Some(self.http1.take().unwrap().into_h2());
                        continue;
                    }
                    return Err(ProtoError::UpgradeHttp2);
                }
                Some(Err(e)) => return Err(e),
                Some(Ok(mut r)) => {
                    // let send_control = r.extensions_mut().remove::<SendControl>();
                    match f(r).await? {
                        Some(res) => {
                            println!("recv res = {:?}", res);
                            // if let Some(mut send_control) = send_control {
                            //     send_control.send_response(res, true)?;
                            // } else {
                                self.send_response(res).await?;
                            // }
                        }
                        None => (),
                    }
                    // async {
                    //     f(r).await
                    // }.await;
                }
            };
        }
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
    }
}
