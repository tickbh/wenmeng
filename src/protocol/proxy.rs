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

use webparse::Url;

use crate::ProtError;

pub enum ProxyScheme {
    Http(Url),
    Https(Url),
    Socks5 {
        addr: SocketAddr,
        auth: Option<(String, String)>,
    },
}

impl ProxyScheme {
    
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
