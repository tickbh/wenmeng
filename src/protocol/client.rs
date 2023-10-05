use crate::{http1::ClientH1Connection, ProtError};
use crate::http2::ClientH2Connection;
use crate::{ProtResult, RecvStream};
use tokio::sync::mpsc::{Receiver, channel, Sender};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::client::TlsStream;
use webparse::{Method, Request, Serialize, Url, WebError, Response};

#[derive(Debug)]
pub struct Builder {
    inner: ProtResult<ClientOption>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            inner: Ok(ClientOption::default()),
        }
    }

    pub fn http2_prior_knowledge(mut self, http2: bool) -> Self {
        self.and_then(move |mut v| {
            v.http2_prior_knowledge = true;
            Ok(v)
        })
    }

    fn and_then<F>(self, func: F) -> Self
    where
        F: FnOnce(ClientOption) -> ProtResult<ClientOption>,
    {
        Builder {
            inner: self.inner.and_then(func),
        }
    }

    pub async fn request<Req: Serialize>(
        self,
        req: &Request<Req>,
    ) -> ProtResult<Client<TcpStream>> {
        let connect = req.get_connect_url();
        if connect.is_none() {
            return Err(ProtError::Extension("unknown connection url"));
        }
        let tcp = TcpStream::connect(connect.unwrap()).await?;
        Ok(Client::<TcpStream>::new(self.inner?, tcp))
    }
}

#[derive(Clone, Debug)]
pub struct ClientOption {
    http2_prior_knowledge: bool,
}

impl Default for ClientOption {
    fn default() -> Self {
        Self {
            http2_prior_knowledge: false,
        }
    }
}

pub struct Client<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    option: ClientOption,
    sender: Sender<Response<RecvStream>>,
    receiver: Option<Receiver<Response<RecvStream>>>,
    req_receiver: Option<Receiver<Request<RecvStream>>>,
    http1: Option<ClientH1Connection<T>>,
    http2: Option<ClientH2Connection<T>>,
}

impl Client<TcpStream> {
    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl<T> Client<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{

    pub fn new(option: ClientOption, stream: T) -> Self {
        let (sender, receiver) = channel(10);
        let mut client = Self {
            option,
            sender,
            receiver: Some(receiver),
            req_receiver: None,
            http1: None,
            http2: None,
        };
        client.http1 = Some(ClientH1Connection::new(stream));
        client
    }

    pub fn split(&mut self) -> ProtResult<(Receiver<Response<RecvStream>>, Sender<Request<RecvStream>>) > {
        if self.receiver.is_none() {
            return Err(ProtError::Extension("receiver error"));
        }
        let (sender, receiver) = channel::<Request<RecvStream>>(10);
        self.req_receiver = Some(receiver);
        Ok((self.receiver.take().unwrap(), sender))
    }

    async fn inner_operate(mut self, req: Request<RecvStream>) -> ProtResult<()> {
        if let Some(h) = &mut self.http1 {
            h.send_request(req).await?;
        }
        loop {
            let result = if let Some(h1) = &mut self.http1 {
                h1.incoming().await
            }
             else if let Some(h2) = &mut self.http2 {
                h2.incoming().await
            }
             else {
                return Ok(());
            };
            match result {
                Ok(None) =>  return Ok(()),
                Err(ProtError::ClientUpgradeHttp2) => {
                    if self.http1.is_some() {
                        self.http2 = Some(self.http1.take().unwrap().into_h2());
                        continue;
                    } else {
                        return Err(ProtError::ClientUpgradeHttp2);
                    }
                }
                Err(e) => return Err(e),
                Ok(Some(r)) => {
                    self.sender.send(r).await?;
                },
            };
        }
        Ok(())
    }

    pub async fn operate(self, req: Request<RecvStream>) -> ProtResult<()> {
        tokio::spawn(async move {
            // let _ = self.operate(req).await;
            let _ = self.inner_operate(req).await;
        });
        Ok(())
    }

    pub async fn send(mut self, req: Request<RecvStream>) -> ProtResult<Receiver<Response<RecvStream>>> {
        let (r, _s) = self.split()?;
        self.operate(req).await?;
        Ok(r)

        // if let Some(h) = &mut self.http1 {
        //     h.send_request(req).await?;
        // }
        // loop {
        //     let result = if let Some(h1) = &mut self.http1 {
        //         h1.incoming(&mut f).await
        //     } else if let Some(h2) = &mut self.http2 {
        //         h2.incoming(&mut f).await
        //     } else {
        //         Ok(Some(true))
        //     };
        //     match result {
        //         Ok(None) | Ok(Some(false)) => continue,
        //         Err(ProtError::ServerUpgradeHttp2(b, r)) => {
        //             if self.http1.is_some() {
        //                 self.http2 = Some(self.http1.take().unwrap().into_h2(b));
        //                 if let Some(r) = r {
        //                     self.http2
        //                         .as_mut()
        //                         .unwrap()
        //                         .handle_request(r, &mut f)
        //                         .await?;
        //                 }
        //                 continue;
        //             } else {
        //                 return Err(ProtError::ServerUpgradeHttp2(b, r));
        //             }
        //         }
        //         Err(e) => return Err(e),
        //         Ok(Some(true)) => return Ok(Some(true)),
        //     };
        // }
        // Ok(())
    }

    pub async fn recv(&mut self) -> ProtResult<Response<RecvStream>> {
        if let Some(recv) = &mut self.receiver {
            if let Some(res) = recv.recv().await {
                Ok(res)
            } else {
                Err(ProtError::Extension("recv close"))
            }
        } else {
            Err(ProtError::Extension("has not recv"))
        }
    }
}
