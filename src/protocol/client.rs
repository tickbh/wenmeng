use std::net::SocketAddr;

use crate::http2::{self, ClientH2Connection};
use crate::{http1::ClientH1Connection, ProtError};
use crate::{ProtResult, RecvStream};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::client::TlsStream;
use webparse::http2::frame::Settings;
use webparse::http2::{DEFAULT_INITIAL_WINDOW_SIZE, DEFAULT_MAX_FRAME_SIZE, HTTP2_MAGIC};
use webparse::{Binary, Method, Request, Response, Serialize, Url, WebError};

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

    pub async fn connect<T>(&self, addr: T) -> ProtResult<TcpStream>
    where T: TryInto<SocketAddr> {
        let url = TryInto::<SocketAddr>::try_into(addr);
        if url.is_err() {
            return Err(ProtError::Extension("unknown connection url"));
        } else {
            let tcp = TcpStream::connect(url.ok().unwrap()).await?;
            Ok(tcp)
        }
    }

    pub async fn connect_tls<T>(&self, addr: T) -> ProtResult<TcpStream>
    where T: TryInto<SocketAddr> {
        let url = TryInto::<SocketAddr>::try_into(addr);
        if url.is_err() {
            return Err(ProtError::Extension("unknown connection url"));
        } else {
            let tcp = TcpStream::connect(url.ok().unwrap()).await?;
            Ok(tcp)
        }
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
        if client.option.http2_prior_knowledge {
            let value = http2::Builder::new()
                .initial_window_size(DEFAULT_INITIAL_WINDOW_SIZE)
                .max_concurrent_streams(100)
                .max_frame_size(DEFAULT_MAX_FRAME_SIZE)
                // .set_enable_push(false)
                .client_connection(stream);
            client.http2 = Some(value);
            client
                .http2
                .as_mut()
                .unwrap()
                .set_handshake_status(Binary::from(HTTP2_MAGIC));
        } else {
            client.http1 = Some(ClientH1Connection::new(stream));
        }
        client
    }

    pub fn split(
        &mut self,
    ) -> ProtResult<(Receiver<Response<RecvStream>>, Sender<Request<RecvStream>>)> {
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
        } else if let Some(h) = &mut self.http2 {
            h.send_request(req).await?;
        }
        loop {
            let result = if let Some(h1) = &mut self.http1 {
                h1.incoming().await
            } else if let Some(h2) = &mut self.http2 {
                h2.incoming().await
            } else {
                return Ok(());
            };
            match result {
                Ok(None) => return Ok(()),
                Err(ProtError::ClientUpgradeHttp2(s)) => {
                    if self.http1.is_some() {
                        self.http2 = Some(self.http1.take().unwrap().into_h2());
                        continue;
                    } else {
                        return Err(ProtError::ClientUpgradeHttp2(s));
                    }
                }
                Err(e) => return Err(e),
                Ok(Some(r)) => {
                    self.sender.send(r).await?;
                }
            };
        }
        Ok(())
    }

    pub async fn send(
        mut self,
        req: Request<RecvStream>,
    ) -> ProtResult<Receiver<Response<RecvStream>>> {
        let (r, _s) = self.split()?;
        tokio::spawn(async move {
            // let _ = self.operate(req).await;
            let e = self.inner_operate(req).await;
            println!("errr = {:?}", e);
        });
        Ok(r)
    }

    pub async fn send2(
        mut self,
        req: Request<RecvStream>,
    ) -> ProtResult<(Receiver<Response<RecvStream>>, Sender<Request<RecvStream>>)> {
        let (r, s) = self.split()?;
        tokio::spawn(async move {
            // let _ = self.operate(req).await;
            let e = self.inner_operate(req).await;
            println!("errr = {:?}", e);
        });
        Ok((r, s))
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
