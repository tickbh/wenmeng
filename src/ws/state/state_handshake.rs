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
// Created Date: 2023/09/14 09:42:25

use crate::{ws::WsCodec, Builder, ProtResult};

use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{Binary, Buf};

pub struct WsStateHandshake {
    /// 默认参数
    builder: Builder,
    /// 当前握手状态
    state: WsHandshaking,
    /// 是否为客户端
    is_client: bool,
    /// 握手日志信息
    span: tracing::Span,
}

/// 握手状态
enum WsHandshaking {
    /// 还未进行握手, 确定http2协议则开始握手
    Wait,
    /// 协议升级信息写入
    Flushing(Flush),
    /// 已完成握手, 不可重复握手
    Done,
}

/// Flush a Sink
struct Flush(Binary);

impl WsStateHandshake {
    pub fn new_server() -> WsStateHandshake {
        WsStateHandshake {
            builder: Builder::new(),
            state: WsHandshaking::Wait,
            is_client: false,
            span: tracing::trace_span!("server_handshake"),
        }
    }

    pub fn new_client() -> WsStateHandshake {
        WsStateHandshake {
            builder: Builder::new(),
            state: WsHandshaking::Wait,
            is_client: true,
            span: tracing::trace_span!("server_handshake"),
        }
    }

    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut WsCodec<T>,
    ) -> Poll<ProtResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        loop {
            match &mut self.state {
                WsHandshaking::Wait => {
                    return Poll::Pending;
                }
                WsHandshaking::Flushing(flush) => {
                    match ready!(flush.poll_handle(cx, codec)) {
                        Ok(_) => {
                            tracing::trace!(flush.poll = %"Ready");
                            self.state = WsHandshaking::Done;
                            continue;
                        }
                        Err(e) => return Poll::Ready(Err(e)),
                    };
                }
                WsHandshaking::Done => {
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
    
    pub fn set_handshake_status(&mut self, binary: Binary, is_client: bool) {
        self.is_client = is_client;
        if binary.is_empty() {
            self.state = WsHandshaking::Done;
        } else {
            self.state = WsHandshaking::Flushing(Flush(binary));
        }
    }
}

impl Flush {
    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut WsCodec<T>,
    ) -> Poll<ProtResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if !self.0.has_remaining() {
            return Poll::Ready(Ok(()));
        }

        loop {
            match ready!(Pin::new(codec.get_mut()).poll_write(cx, self.0.chunk())) {
                Ok(n) => {
                    self.0.advance(n);
                }
                Err(e) => return Poll::Ready(Err(e.into())),
            }
            if !self.0.has_remaining() {
                return Poll::Ready(Ok(()));
            }
        }
    }
}

unsafe impl Send for WsStateHandshake {}

unsafe impl Sync for WsStateHandshake {}
