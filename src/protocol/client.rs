use std::io;

use std::sync::Arc;

use crate::http2::{self, ClientH2Connection};
use crate::{http1::ClientH1Connection, ProtError};
use crate::{ProtResult, RecvStream};
use rustls::{RootCertStore, ClientConfig};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use webparse::http2::frame::Settings;
use webparse::http2::{DEFAULT_INITIAL_WINDOW_SIZE, DEFAULT_MAX_FRAME_SIZE, HTTP2_MAGIC};
use webparse::{Binary, Request, Response, Serialize, Url};


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

    pub fn http2_only(self, http2: bool) -> Self {
        self.and_then(move |mut v| {
            v.http2_only = http2;
            Ok(v)
        })
    }
    
    pub fn http2(self, http2: bool) -> Self {
        self.and_then(move |mut v| {
            v.http2 = http2;
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

    pub fn value(self) -> ProtResult<ClientOption> {
        self.inner
    }

    pub async fn connect<T>(self, url: T) -> ProtResult<Client<TcpStream>>
    where T: TryInto<Url> {
        let url = TryInto::<Url>::try_into(url);
        println!("url = {:?}", url.as_ref().ok());
        if url.is_err() {
            return Err(ProtError::Extension("unknown connection url"));
        } else {
            let tcp = TcpStream::connect(url.ok().unwrap().get_connect_url().unwrap()).await?;
            Ok(Client::<TcpStream>::new(self.inner?, tcp))
        }
    }

    pub async fn connect_tls<T>(self, url: T) -> ProtResult<Client<TlsStream<TcpStream>>>
    where T: TryInto<Url> {
        let mut option = self.inner?;
        let url = TryInto::<Url>::try_into(url);
        if url.is_err() {
            return Err(ProtError::Extension("unknown connection url"));
        } else {
            let url = url.ok().unwrap();
            let connect = url.get_connect_url();
            let domain = url.domain;
            if domain.is_none() || connect.is_none() {
                return Err(ProtError::Extension("unknown connection domain"));
            }
            println!("domain = {:?}", domain);
            println!("connect = {:?}", connect);
            let mut root_store = RootCertStore::empty();
            root_store.add_trust_anchors(
                webpki_roots::TLS_SERVER_ROOTS
                    .iter()
                    .map(|ta| {
                        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                            ta.subject,
                            ta.spki,
                            ta.name_constraints,
                        )
                    }),
            );
            let mut config = ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            config.alpn_protocols = option.get_alpn_protocol();
            let tls_client = Arc::new(config);
            let connector = TlsConnector::from(tls_client);

            let stream = TcpStream::connect(&connect.unwrap()).await?;
            // 这里的域名只为认证设置
            let domain =
                rustls::ServerName::try_from(&*domain.unwrap())
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

            let outbound = connector.connect(domain, stream).await?;
            let aa = outbound.get_ref().1.alpn_protocol();
            if aa.is_none() {
                return Err(ProtError::Extension("not support protocol"));
            }

            if aa == Some(&ClientOption::H2_PROTOCOL) {
                option.http2_only = true;
            }

            println!("aaaaaaaaaaa == {:?}", aa);
            println!("aaaaaaaaaaa == {:?}", String::from_utf8_lossy(aa.unwrap()));
            Ok(Client::new(option, outbound))
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClientOption {
    http2_only: bool,
    http2: bool,
    settings: Settings,
}

impl ClientOption {
    pub const H2_PROTOCOL: [u8; 2] = [104, 50];
    pub fn get_alpn_protocol(&self) -> Vec<Vec<u8>> {
        let mut ret = vec![];
        if self.http2_only {
            ret.push(Self::H2_PROTOCOL.to_vec());
        } else {
            ret.push("http/1.1".as_bytes().to_vec());
            if self.http2 {
                ret.push(Self::H2_PROTOCOL.to_vec());
            }
        }
        ret
    }

    pub fn get_http2_setting(&self) -> String {
        self.settings.encode_http_settings()
    }
}

impl Default for ClientOption {
    fn default() -> Self {
        Self {
            http2_only: false,
            http2: true,
            settings: Default::default(),
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
        if client.option.http2_only {
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

    async fn send_req(&mut self, req: Request<RecvStream>) -> ProtResult<()> {
        if let Some(h) = &mut self.http1 {
            h.send_request(req).await?;
        } else if let Some(h) = &mut self.http2 {
            h.send_request(req).await?;
        }
        Ok(())
    }

    async fn inner_operate(mut self, req: Request<RecvStream>) -> ProtResult<()> {
        self.send_req(req).await?;

        async fn http1_wait<T>(connection: &mut Option<ClientH1Connection<T>>) -> Option<ProtResult<Option<Response<RecvStream>>>>
        where T: AsyncRead + AsyncWrite + Unpin {
            if connection.is_some() {
                Some(connection.as_mut().unwrap().incoming().await)
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                None
            }
        }
        
        async fn http2_wait<T>(connection: &mut Option<ClientH2Connection<T>>) -> Option<ProtResult<Option<Response<RecvStream>>>>
        where T: AsyncRead + AsyncWrite + Unpin {
            if connection.is_some() {
                Some(connection.as_mut().unwrap().incoming().await)
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                None
            }
        }
        
        async fn req_receiver(req_receiver: &mut Option<Receiver<Request<RecvStream>>>) -> Option<Request<RecvStream>>
        {
            if req_receiver.is_some() {
                req_receiver.as_mut().unwrap().recv().await
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                None
            }
        }

        loop {
            let v = tokio::select! {
                r = http1_wait(&mut self.http1) => {
                    println!("aaaaaa {:?}", r);
                    r
                }
                r = http2_wait(&mut self.http2) => {
                    println!("bbbb {:?}", r);
                    r
                }
                req = req_receiver(&mut self.req_receiver) => {
                    println!("cccc");
                    if let Some(req) = req {
                        self.send_req(req).await?;
                    } else {
                        self.req_receiver = None;
                    }
                    continue;
                }
            };
            if v.is_none() {
                return Ok(());
            }
            let result = v.unwrap();
            match result {
                Ok(None) => return Ok(()),
                Err(ProtError::ClientUpgradeHttp2(s)) => {
                    if self.http1.is_some() {
                        self.http2 = Some(self.http1.take().unwrap().into_h2(s));
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
    }

    fn rebuild_request(&mut self, req: &mut Request<RecvStream>) {
        // 支持http2且当前为http1尝试升级
        if self.option.http2 {
            if let Some(_) = &self.http1 {
                let header = req.headers_mut();
                header.insert("Connection", "Upgrade, HTTP2-Settings");
                header.insert("Upgrade", "h2c");
                header.insert("HTTP2-Settings", self.option.get_http2_setting());
            }
        }
    }

    pub async fn send(
        mut self,
        mut req: Request<RecvStream>,
    ) -> ProtResult<Receiver<Response<RecvStream>>> {
        self.rebuild_request(&mut req);
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
        mut req: Request<RecvStream>,
    ) -> ProtResult<(Receiver<Response<RecvStream>>, Sender<Request<RecvStream>>)> {
        self.rebuild_request(&mut req);
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
