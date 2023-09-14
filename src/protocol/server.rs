use std::any::{Any, TypeId};

use futures_core::{Future};
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{http::http2::frame::StreamIdentifier, Binary, Request, Response, Serialize};

use crate::{H2Connection, ProtError, ProtResult, RecvStream};

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

    pub async fn send_response<R>(
        &mut self,
        res: Response<R>,
        stream_id: Option<StreamIdentifier>,
    ) -> ProtResult<()>
    where
        RecvStream: From<R>,
        R: Serialize,
    {
        let _result = if let Some(h1) = &mut self.http1 {
            h1.send_response(res.into_type::<RecvStream>()).await?;
        } else if let Some(h2) = &mut self.http2 {
            if let Some(stream_id) = stream_id {
                let _recv = RecvStream::only(Binary::new());
                // let v = res as Response<RecvStream>;
                let res = res.into_type::<RecvStream>();
                // let (mut res, mut r) = res.into(recv);

                // let r = unsafe {
                // (&mut **res.body_mut() as &mut (dyn Any + 'static)).downcast_mut()
                // Box::new(res).as_mut()
                // }
                h2.send_response(res, stream_id).await?;
            }
        };

        Ok(())
    }

    pub async fn req_into_type<Req>(mut r: Request<RecvStream>) -> Request<Req>
    where
        Req: From<RecvStream>,
        Req: Serialize + Any,
    {
        if TypeId::of::<Req>() == TypeId::of::<RecvStream>() {
            r.into_type::<Req>()
        } else {
            if !r.body().is_end() {
                let _ = r.body_mut().wait_all().await;
            }
            r.into_type::<Req>()
        }
    }

    
    pub async fn try_wait_req<Req>(r: &mut Request<RecvStream>) -> ProtResult<()>
    where
        Req: From<RecvStream>,
        Req: Serialize,
    {
        if !r.body().is_end() {
            let _ = r.body_mut().wait_all().await;
        }
        Ok(())
    }

    pub async fn incoming<F, Fut, Res, Req>(&mut self, mut f: F) -> ProtResult<Option<bool>>
    where
    F: FnMut(Request<Req>) -> Fut,
    Fut: Future<Output = ProtResult<Option<Response<Res>>>>,
    Req: From<RecvStream>,
    Req: Serialize + Any,
    RecvStream: From<Res>,
    Res: Serialize + Any
    {
        let xx = TypeId::of::<Req>();
        let xx1 = TypeId::of::<RecvStream>();
        println!("111 = {:?} 222 = {:?}", xx, xx1);
        loop {
            let result = if let Some(h1) = &mut self.http1 {
                h1.incoming(&mut f).await
            } else if let Some(h2) = &mut self.http2 {
                h2.incoming(&mut f).await
            } else {
                Ok(Some(true))
            };
            // println!("test: result = {:?}", result);
            match result {
                Ok(None) | Ok(Some(false)) => continue,
                Err(ProtError::UpgradeHttp2(b, r)) => {
                    if self.http1.is_some() {
                        self.http2 = Some(self.http1.take().unwrap().into_h2(b));
                        if let Some(r) = r {
                            self.http2
                                .as_mut()
                                .unwrap()
                                .handle_request(r, &mut f)
                                .await?;
                        }
                        continue;
                    } else {
                        return Err(ProtError::UpgradeHttp2(b, r));
                    }
                }
                Err(e) => return Err(e),
                Ok(Some(true)) => return Ok(Some(true)),
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
