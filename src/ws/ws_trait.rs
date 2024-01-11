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
use webparse::{
    ws::{CloseData, OwnedMessage},
};

use crate::{ProtError, ProtResult, RecvRequest, RecvResponse};

use super::{WsHandshake, WsOption};

#[async_trait]
pub trait WsTrait: Send {
    /// 通过请求连接构建出返回的握手连接信息
    #[inline]
    async fn on_request(&mut self, req: &RecvRequest) -> ProtResult<RecvResponse> {
        // warn!("Handler received request:\n{}", req);
        WsHandshake::build_request(req)
    }

    /// 握手完成后之后的回调,服务端返回了Response之后就认为握手成功
    async fn on_open(&mut self, shake: WsHandshake) -> ProtResult<Option<WsOption>>;

    /// 接受到远端的关闭消息
    async fn on_close(&mut self, _reason: &Option<CloseData>) {}

    /// 服务内部出现了错误代码
    async fn on_error(&mut self, _err: ProtError) {}

    /// 收到来在远端的ping消息, 默认返回pong消息
    async fn on_ping(&mut self, val: Vec<u8>) -> ProtResult<Option<OwnedMessage>> {
        return Ok(Some(OwnedMessage::Pong(val)));
    }

    /// 收到来在远端的pong消息, 默认不做任何处理, 可自定义处理如ttl等
    async fn on_pong(&mut self, _val: Vec<u8>) -> ProtResult<()> {
        Ok(())
    }

    /// 收到来在远端的message消息, 必须覆写该函数
    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()>;

    /// 定时器定时按间隔时间返回
    async fn on_interval(&mut self, _option: &mut Option<WsOption>) -> ProtResult<()> {
        Ok(())
    }
    
    /// 将当前trait转化成Any,仅需当需要重新获取回调处理的时候进行处理
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    /// 将当前trait转化成mut Any,仅需当需要重新获取回调处理的时候进行处理
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}
