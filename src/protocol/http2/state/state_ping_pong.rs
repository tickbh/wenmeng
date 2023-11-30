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

use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::http::http2::frame::{Ping, Frame};

use crate::{protocol::http2::codec::Codec, ProtResult};

pub struct StatePingPong {
    ping: Option<Ping>,
}

impl StatePingPong {
    pub fn new() -> Self {
        StatePingPong { ping: None }
    }

    pub fn receive(&mut self, ping: Ping) {
        self.ping = Some(ping);
    }

    
    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>
    ) -> Poll<ProtResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if let Some(frame) = self.ping.take() {
            if !codec.poll_ready(cx)?.is_ready() {
                self.ping = Some(frame);
                return Poll::Pending;
            }

            let pong = frame.ret_pong();
            codec.send_frame(Frame::Ping(pong))?;
            return Poll::Ready(Ok(()));
        }
        return Poll::Ready(Ok(()));
    }
}
