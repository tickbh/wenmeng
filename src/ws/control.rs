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
// Created Date: 2024/01/02 10:51:49

use std::{task::{Context, Poll, ready}, pin::Pin};

use futures::Stream;
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{Binary, OwnedMessage};

use crate::ProtResult;

use super::{state::{WsStateGoAway, WsStateHandshake, WsStatePingPong}, WsCodec};

pub(crate) struct Control {
    handshake: WsStateHandshake,
    goaway: WsStateGoAway,
    pingpong: WsStatePingPong,

    is_client: bool,
}

impl Control {
    pub fn new() -> Self {
        Self {
            handshake: WsStateHandshake::new_server(),
            goaway: WsStateGoAway::new(),
            pingpong: WsStatePingPong::new(),
            is_client: false,
        }
    }

    pub fn set_handshake_status(&mut self, binary: Binary, is_client: bool) {
        self.is_client = is_client;
        self.handshake.set_handshake_status(binary, is_client);
    }

    pub fn poll_request<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut WsCodec<T>,
    ) -> Poll<Option<ProtResult<OwnedMessage>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ready!(self.handshake.poll_handle(cx, codec))?;
        
        match Pin::new(&mut *codec).poll_next(cx) {
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Ready(Some(Ok(msg))) => {
                println!("msg = {:?}", msg);
                return Poll::Ready(Some(Ok(msg)));
            },
            Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
            Poll::Pending => return Poll::Pending,
        }
        // let mut has_change;
        Poll::Pending
    }
}
