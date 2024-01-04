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
// Created Date: 2024/01/04 11:03:00

use async_trait::async_trait;
use webparse::{Response, OwnedMessage};

use crate::{ProtResult, RecvRequest, RecvResponse, ProtError};

use super::Handshake;

#[async_trait]
pub trait WsTrait {
    #[inline]
    fn on_request(&mut self, req: &RecvRequest) -> ProtResult<RecvResponse> {
        // warn!("Handler received request:\n{}", req);
        let mut response = Response::builder()
            .status(101)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Accept", "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")
            .header("Sec-WebSocket-Protocol", "chat")
            .body(())
            .unwrap();
        Ok(response.into_type())
    }

    fn on_open(&mut self, shake: Handshake) -> ProtResult<()> {
        Ok(())
    }

    
    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()>;

    async fn on_close(&mut self, reason: &str) {
    }
    
    async fn on_error(&mut self, err: ProtError) {
    }
}
