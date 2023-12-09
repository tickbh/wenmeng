// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2023/12/07 03:05:04

use lazy_static::lazy_static;
use std::{net::SocketAddr, os::unix::ffi::OsStrExt, env, collections::HashSet};

use tokio::{net::TcpStream, io::{AsyncRead, AsyncWrite}};
use webparse::{Url, HeaderValue, BinaryMut, Scheme};

use crate::{ProtError, ProtResult, MaybeHttpsStream};



#[derive(Debug, Clone)]
pub enum ProxyScheme {
    Http {
        addr: SocketAddr,
        auth: Option<(String, String)>,
    },
    Https {
        addr: SocketAddr,
        auth: Option<(String, String)>,
    },
    Socks5 {
        addr: SocketAddr,
        auth: Option<(String, String)>,
    },
}

async fn tunnel<T>(
    mut conn: T,
    host: String,
    port: u16,
    user_agent: Option<HeaderValue>,
    auth: Option<HeaderValue>,
) -> ProtResult<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = format!(
        "\
         CONNECT {0}:{1} HTTP/1.1\r\n\
         Host: {0}:{1}\r\n\
         ",
        host, port
    )
    .into_bytes();

    // user-agent
    if let Some(user_agent) = user_agent {
        buf.extend_from_slice(b"User-Agent: ");
        buf.extend_from_slice(user_agent.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }

    // proxy-authorization
    if let Some(value) = auth {
        log::debug!("tunnel to {}:{} using basic auth", host, port);
        buf.extend_from_slice(b"Proxy-Authorization: ");
        buf.extend_from_slice(value.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }

    // headers end
    buf.extend_from_slice(b"\r\n");

    conn.write_all(&buf).await?;

    let mut buf = [0; 8192];
    let mut pos = 0;

    loop {
        let n = conn.read(&mut buf[pos..]).await?;

        if n == 0 {
            return Err(ProtError::Extension("eof error"));
        }
        pos += n;

        let recvd = &buf[..pos];
        if recvd.starts_with(b"HTTP/1.1 200") || recvd.starts_with(b"HTTP/1.0 200") {
            if recvd.ends_with(b"\r\n\r\n") {
                return Ok(conn);
            }
            if pos == buf.len() {
                return Err(ProtError::Extension("proxy headers too long for tunnel"));
            }
        // else read more
        } else if recvd.starts_with(b"HTTP/1.1 407") {
            return Err(ProtError::Extension("proxy authentication required"));
        } else {
            return Err(ProtError::Extension("unsuccessful tunnel"));
        }
    }
}

fn insert_from_env(proxies: &mut Vec<ProxyScheme>, scheme: Scheme, key: &str) -> bool {
    if let Ok(val) = env::var(key) {
        if let Ok(proxy) = ProxyScheme::try_from(&*val) {
            if scheme.is_http() {
                if let Ok(proxy) = proxy.trans_http() {
                    proxies.push(proxy);
                    return true;
                }
            } else {
                if let Ok(proxy) = proxy.trans_https() {
                    proxies.push(proxy);
                    return true;
                }
            }
        }
    }
    false
}

fn get_from_environment() -> Vec<ProxyScheme> {
    let mut proxies = vec![];

    if !insert_from_env(&mut proxies, Scheme::Http, "HTTP_PROXY") {
        insert_from_env(&mut proxies, Scheme::Http, "http_proxy");
    }

    if !insert_from_env(&mut proxies, Scheme::Https, "HTTPS_PROXY") {
        insert_from_env(&mut proxies, Scheme::Https, "https_proxy");
    }

    if !(insert_from_env(&mut proxies, Scheme::Http, "ALL_PROXY")
        && insert_from_env(&mut proxies, Scheme::Https, "ALL_PROXY"))
    {
        insert_from_env(&mut proxies, Scheme::Http, "all_proxy");
        insert_from_env(&mut proxies, Scheme::Https, "all_proxy");
    }

    proxies
}

impl ProxyScheme {

    pub fn get_env_proxies() -> &'static Vec<ProxyScheme> {
        lazy_static! {
            static ref ENV_PROXIES: Vec<ProxyScheme> = get_from_environment();
        }
        &ENV_PROXIES
    }

    pub fn get_env_no_proxy() -> &'static HashSet<String> {
        lazy_static! {
            static ref ENV_NO_PROXY: HashSet<String> = {
                let mut hash = HashSet::new();
                hash.insert("localhost".to_string());
                hash.insert("127.0.0.1".to_string());
                hash.insert("::1".to_string());
                fn insert_no_proxy(all_hash: &mut HashSet<String>, key: &str) -> bool {
                    if let Ok(val) = env::var(key) {
                        let all = val.split(",").collect::<Vec<&str>>();
                        for one in all {
                            all_hash.insert(one.trim().to_string());
                        }
                        return true
                    }
                    false
                }
                if !insert_no_proxy(&mut hash, "NO_PROXY") {
                    insert_no_proxy(&mut hash, "no_proxy");
                }
                hash
            };
        }
        &ENV_NO_PROXY
    }

    pub fn is_no_proxy(host: &String) -> bool {
        let hash = Self::get_env_no_proxy();
        println!("hashs = {:?}", hash);
        hash.contains(host)
    }

    pub async fn connect(&self, url:&Url) -> ProtResult<Option<TcpStream>> {
        match self {
            ProxyScheme::Http {addr, auth : _} => {
                println!("connect = {}", addr);
                let tcp = TcpStream::connect(addr).await?;
                if url.scheme.is_https() {
                    let tcp = tunnel(tcp, url.domain.clone().unwrap_or_default(), url.port.unwrap_or(443), None, None).await?;
                    return Ok(Some(tcp));
                } else {
                    return Ok(Some(tcp));
                }
            },
            ProxyScheme::Https {addr, auth: _ }  => {
                if !url.scheme.is_https() {
                    return Ok(None);
                }
                let tcp = TcpStream::connect(addr).await?;
                let tcp = tunnel(tcp, url.domain.clone().unwrap_or_default(), url.port.unwrap_or(443), None, None).await?;
                return Ok(Some(tcp));
            },
            ProxyScheme::Socks5 { addr, auth } => {
                let tcp = TcpStream::connect(addr).await?;
                let tcp = Self::socks5_connect(tcp, &url, auth).await?;
                return Ok(Some(tcp))
            },
        }
    }

    fn trans_http(self) -> ProtResult<Self> {
        match self {
            ProxyScheme::Http { addr, auth } => {
                Ok(ProxyScheme::Http { addr, auth })
            }
            ProxyScheme::Https { addr, auth } => {
                Ok(ProxyScheme::Http { addr, auth })
            }
            _ => {
                Err(ProtError::Extension("unknow type"))
            }
        }
    }


    fn trans_https(self) -> ProtResult<Self> {
        match self {
            ProxyScheme::Http { addr, auth } => {
                Ok(ProxyScheme::Https { addr, auth })
            }
            ProxyScheme::Https { addr, auth } => {
                Ok(ProxyScheme::Https { addr, auth })
            }
            _ => {
                Err(ProtError::Extension("unknow type"))
            }
        }
    }

    async fn socks5_connect<T>(
        mut conn: T,
        url: &Url,
        auth: &Option<(String, String)>,
    ) -> ProtResult<T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use webparse::BufMut;
        let mut binary = BinaryMut::new();
        let mut data = vec![0;1024];
        if let Some(_auth) = auth {
            conn.write_all(&[5, 1, 2]).await?;
        } else {
            conn.write_all(&[5, 0]).await?;
        }

        conn.read_exact(&mut data[..2]).await?;
        if data[0] != 5 {
            return Err(ProtError::Extension("socks5 error"));
        }
        match data[1] {
            2 => {
                let (user, pass) = auth.as_ref().unwrap();
                binary.put_u8(1);
                binary.put_u8(user.as_bytes().len() as u8);
                binary.put_slice(user.as_bytes());
                binary.put_u8(pass.as_bytes().len() as u8);
                binary.put_slice(pass.as_bytes());
                conn.write_all(binary.as_slice()).await?;

                conn.read_exact(&mut data[..2]).await?;
                if data[0] != 1 || data[1] != 0 {
                    return Err(ProtError::Extension("user password error"));
                }

                binary.clear();
            }
            0 => {},
            _ => {
                return Err(ProtError::Extension("no method for auth"));
            }
        }

        binary.put_slice(&[5, 1, 0, 3]);
        let domain = url.domain.as_ref().unwrap();
        let port = url.port.unwrap_or(80);
        binary.put_u8(domain.as_bytes().len() as u8);
        binary.put_slice(domain.as_bytes());
        binary.put_u16(port);
        conn.write_all(&binary.as_slice()).await?;
        conn.read_exact(&mut data[..10]).await?;
        if data[0] != 5 {
            return Err(ProtError::Extension("socks5 error"));
        }
        if data[1] != 0 {
            return Err(ProtError::Extension("network error"));
        }
        Ok(conn)
    }
}

impl TryFrom<&str> for ProxyScheme {
    type Error = ProtError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let url = Url::try_from(value)?;
        let (addr, auth) = if let Some(connect) = url.get_connect_url() {
            let addr = connect
                .parse::<SocketAddr>()
                .map_err(|_| ProtError::Extension("unknow parse"))?;
            let auth = if url.username.is_some() && url.password.is_some() {
                Some((url.username.unwrap(), url.password.unwrap()))
            } else {
                None
            };
            (addr, auth)
        } else {
            return Err(ProtError::Extension("unknow addr"))
        };
        match &url.scheme {
            webparse::Scheme::Http => Ok(ProxyScheme::Http {
                addr, auth
            }),
            webparse::Scheme::Https => Ok(ProxyScheme::Https {
                addr, auth
            }),
            webparse::Scheme::Extension(s) if s == "socks5" => {
                Ok(ProxyScheme::Socks5 { addr, auth })
            }
            _ => Err(ProtError::Extension("unknow scheme")),
        }
    }
}
