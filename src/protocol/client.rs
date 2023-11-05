use std::io;

use std::sync::Arc;
use std::time::Duration;

use crate::http2::{self, ClientH2Connection};
use crate::{http1::ClientH1Connection, ProtError};
use crate::{ProtResult, RecvStream};
use rustls::{ClientConfig, RootCertStore};
use tokio::net::ToSocketAddrs;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
use webparse::http2::frame::Settings;
use webparse::http2::{DEFAULT_INITIAL_WINDOW_SIZE, DEFAULT_MAX_FRAME_SIZE, HTTP2_MAGIC};
use webparse::{Binary, Request, Response, Url};

#[derive(Debug)]
pub struct Builder {
    inner: ClientOption,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            inner: ClientOption::default(),
        }
    }

    pub fn http2_only(mut self, http2_only: bool) -> Self {
        self.inner.http2_only = http2_only;
        self
    }

    pub fn http2(mut self, http2: bool) -> Self {
        self.inner.http2 = http2;
        self
    }

    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.inner.connect_timeout = Some(connect_timeout);
        self
    }

    
    pub fn read_timeout(mut self, read_timeout: Duration) -> Self {
        self.inner.read_timeout = Some(read_timeout);
        self
    }

    
    pub fn write_timeout(mut self, write_timeout: Duration) -> Self {
        self.inner.write_timeout = Some(write_timeout);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    pub fn value(self) -> ClientOption {
        self.inner
    }

    pub async fn connect_by_stream(self, stream: TcpStream) -> ProtResult<Client<TcpStream>> {
        Ok(Client::<TcpStream>::new(self.inner, stream))
    }

    async fn inner_connect<A: ToSocketAddrs>(&self, addr: A) -> ProtResult<TcpStream> {
        if let Some(connect) = &self.inner.connect_timeout {
            match tokio::time::timeout(*connect, TcpStream::connect(addr)).await {
                Ok(v) => {
                    let tcp = v?;
                    Ok(tcp)
                }
                Err(_) => return Err(ProtError::Extension("timeout")),
            }

        } else {
            let tcp = TcpStream::connect(addr).await?;
            Ok(tcp)
        }
    }

    pub async fn connect<T>(self, url: T) -> ProtResult<Client<TcpStream>>
    where
        T: TryInto<Url>,
    {
        let url = TryInto::<Url>::try_into(url);
        if url.is_err() {
            return Err(ProtError::Extension("unknown connection url"));
        } else {
            let tcp = self.inner_connect(url.ok().unwrap().get_connect_url().unwrap()).await?;
            Ok(Client::<TcpStream>::new(self.inner, tcp))
        }
    }

    pub async fn connect_tls_by_stream<T>(
        self,
        stream: TcpStream,
        url: T,
    ) -> ProtResult<Client<TlsStream<TcpStream>>>
    where
        T: TryInto<Url>,
    {
        let mut option = self.inner;
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
            let mut root_store = RootCertStore::empty();
            root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }));
            let mut config = ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            config.alpn_protocols = option.get_alpn_protocol();
            let tls_client = Arc::new(config);
            let connector = TlsConnector::from(tls_client);

            // 这里的域名只为认证设置
            let domain = rustls::ServerName::try_from(&*domain.unwrap())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

            let outbound = connector.connect(domain, stream).await?;
            let aa = outbound.get_ref().1.alpn_protocol();
            if aa.is_none() {
                return Err(ProtError::Extension("not support protocol"));
            }

            if aa == Some(&ClientOption::H2_PROTOCOL) {
                option.http2_only = true;
            }

            Ok(Client::new(option, outbound))
        }
    }

    pub async fn connect_tls<T>(mut self, url: T) -> ProtResult<Client<TlsStream<TcpStream>>>
    where
        T: TryInto<Url>,
    {
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
            root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }));
            let mut config = ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            config.alpn_protocols = self.inner.get_alpn_protocol();
            let tls_client = Arc::new(config);
            let connector = TlsConnector::from(tls_client);

            let stream = self.inner_connect(&connect.unwrap()).await?;
            // 这里的域名只为认证设置
            let domain = rustls::ServerName::try_from(&*domain.unwrap())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

            let outbound = connector.connect(domain, stream).await?;
            let aa = outbound.get_ref().1.alpn_protocol();
            if aa.is_none() {
                return Err(ProtError::Extension("not support protocol"));
            }

            if aa == Some(&ClientOption::H2_PROTOCOL) {
                self.inner.http2_only = true;
            }

            Ok(Client::new(self.inner, outbound))
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClientOption {
    http2_only: bool,
    http2: bool,
    settings: Settings,
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    timeout: Option<Duration>,
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
            connect_timeout: None,
            read_timeout: None,
            write_timeout: None,
            timeout: None,
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
            client.http1 = Some(client.build_client_h1_connection(stream));
        }
        client
    }

    fn build_client_h1_connection(&self, stream: T) -> ClientH1Connection<T> {
        let mut client = ClientH1Connection::new(stream);
        client.set_read_timeout(self.option.read_timeout);
        client.set_write_timeout(self.option.write_timeout);
        client.set_timeout(self.option.timeout);
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

    fn send_req(&mut self, req: Request<RecvStream>) -> ProtResult<()> {
        if let Some(h) = &mut self.http1 {
            h.send_request(req)?;
        } else if let Some(h) = &mut self.http2 {
            h.send_request(req)?;
        }
        Ok(())
    }

    pub async fn wait_operate(mut self) -> ProtResult<()> {
        async fn http1_wait<T>(
            connection: &mut Option<ClientH1Connection<T>>,
        ) -> Option<ProtResult<Option<Response<RecvStream>>>>
        where
            T: AsyncRead + AsyncWrite + Unpin,
        {
            if connection.is_some() {
                Some(connection.as_mut().unwrap().incoming().await)
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                None
            }
        }

        async fn http2_wait<T>(
            connection: &mut Option<ClientH2Connection<T>>,
        ) -> Option<ProtResult<Option<Response<RecvStream>>>>
        where
            T: AsyncRead + AsyncWrite + Unpin,
        {
            if connection.is_some() {
                Some(connection.as_mut().unwrap().incoming().await)
            } else {
                let pend = std::future::pending();
                let () = pend.await;
                None
            }
        }

        async fn req_receiver(
            req_receiver: &mut Option<Receiver<Request<RecvStream>>>,
        ) -> Option<Request<RecvStream>> {
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
                    r
                }
                r = http2_wait(&mut self.http2) => {
                    r
                }
                req = req_receiver(&mut self.req_receiver) => {
                    if let Some(req) = req {
                        self.send_req(req)?;
                    } else {
                        self.req_receiver = None;
                    }
                    continue;
                }
                () = self.sender.closed() => {
                    log::trace!("接收方被断开, 此时关闭Client");
                    return Ok(());
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

    async fn inner_operate(mut self, req: Request<RecvStream>) -> ProtResult<()> {
        self.send_req(req)?;
        self.wait_operate().await
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
            if let Err(e) = self.inner_operate(req).await {
                println!("http数据请求时发生错误: {:?}", e);
            }
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
            if let Err(e) = self.inner_operate(req).await {
                println!("http数据请求时发生错误: {:?}", e);
            }
        });
        Ok((r, s))
    }

    pub async fn send_now(
        mut self,
        mut req: Request<RecvStream>,
    ) -> ProtResult<Response<RecvStream>> {
        self.rebuild_request(&mut req);
        let (mut r, _) = self.split()?;
        // let _ = self.operate(req).await;
        self.inner_operate(req).await?;
        if let Some(s) = r.recv().await {
            return Ok(s);
        } else {
            return Err(ProtError::Extension("unknow response"));
        }
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


// impl<T> Drop for Client<T>
// where
//     T: AsyncRead + AsyncWrite + Unpin + Send + 'static, {
//         fn drop(&mut self) {
//             println!("drop client!!!!!!!");
//             // drop(self.)
//         }
//     }