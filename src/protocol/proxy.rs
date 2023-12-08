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

use std::net::SocketAddr;

use tokio::{net::TcpStream, io::{AsyncRead, AsyncWrite}};
use webparse::{Url, HeaderValue};

use crate::{ProtError, ProtResult, MaybeHttpsStream};

pub enum ProxyScheme {
    Http(Url),
    Https(Url),
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


async fn socks5_connect<T>(
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

impl ProxyScheme {

    pub async fn connect(&self) -> ProtResult<TcpStream> {
        match self {
            ProxyScheme::Http(url) => {
                if let Some(connect) = url.get_connect_url() {
                    return Ok(TcpStream::connect(connect).await?)
                }
            },
            ProxyScheme::Https(url) => {
                if let Some(connect) = url.get_connect_url() {
                    let tcp = TcpStream::connect(connect).await?;
                    let tcp = tunnel(tcp, url.domain.clone().unwrap_or_default(), url.port.unwrap_or(80), None, None).await?;
                    return Ok(tcp);
                }
            },
            ProxyScheme::Socks5 { addr, auth } => {
                let tcp = TcpStream::connect(addr).await?;

            },
        }

        todo!()
    }
}

impl TryFrom<&str> for ProxyScheme {
    type Error = ProtError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let url = Url::try_from(value)?;
        match &url.scheme {
            webparse::Scheme::Http => Ok(ProxyScheme::Http(url)),
            webparse::Scheme::Https => Ok(ProxyScheme::Https(url)),
            webparse::Scheme::Extension(s) if s == "socks5" => {
                if let Some(connect) = url.get_connect_url() {
                    let addr = connect
                        .parse::<SocketAddr>()
                        .map_err(|_| ProtError::Extension("unknow parse"))?;
                    let auth = if url.username.is_some() && url.password.is_some() {
                        Some((url.username.unwrap(), url.password.unwrap()))
                    } else {
                        None
                    };
                    Ok(ProxyScheme::Socks5 { addr, auth })
                } else {
                    Err(ProtError::Extension("unknow addr"))
                }
            }
            _ => Err(ProtError::Extension("unknow scheme")),
        }
    }
}
