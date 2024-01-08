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

use std::any::Any;

use async_trait::async_trait;
use webparse::{OwnedMessage, CloseData, WebError, WsError};

use crate::{ProtError, ProtResult, RecvRequest, RecvResponse};

use super::{WsHandshake, WsOption};

#[async_trait]
pub trait WsTrait: Send {
    #[inline]
    fn on_request(&mut self, req: &RecvRequest) -> ProtResult<RecvResponse> {
        // warn!("Handler received request:\n{}", req);
        WsHandshake::build_request(req)
    }

    fn on_open(&mut self, shake: WsHandshake) -> ProtResult<Option<WsOption>>;

    async fn on_close(&mut self, reason: Option<CloseData>) {}

    async fn on_error(&mut self, err: ProtError) {}

    async fn on_ping(&mut self, val: Vec<u8>) -> ProtResult<OwnedMessage> {
        return Ok(OwnedMessage::Pong(val));
    }
    
    async fn on_pong(&mut self, val: Vec<u8>) {
    }

    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()>;

    async fn on_interval(&mut self, option: &mut Option<WsOption>) -> ProtResult<()> {
        Ok(())
    }

    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}
