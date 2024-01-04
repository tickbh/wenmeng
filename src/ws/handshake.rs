// Copyright 2022 - 2024 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2024/01/04 11:12:31

use std::net::SocketAddr;

use crate::{RecvRequest, RecvResponse};

pub struct Handshake {
    /// The HTTP request sent to begin the handshake.
    pub request: RecvRequest,
    /// The HTTP response from the server confirming the handshake.
    pub response: RecvResponse,
    /// The socket address of the other endpoint. This address may
    /// be an intermediary such as a proxy server.
    pub peer_addr: Option<SocketAddr>,
    /// The socket address of this endpoint.
    pub local_addr: Option<SocketAddr>,
}

impl Handshake {
    pub fn new(
        request: RecvRequest,
        response: RecvResponse,
        peer_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            request,
            response,
            peer_addr,
            local_addr: None,
        }
    }
}
