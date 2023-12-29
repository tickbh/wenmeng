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
use webparse::http::http2::frame::{GoAway, Reason, Frame};

use crate::{http2::codec::Codec, ProtResult};

pub struct StateGoAway {
    close_now: bool,
    goaway: Option<GoAway>,
    reason: Reason,
}

impl StateGoAway {
    pub fn new() -> Self {
        StateGoAway {
            close_now: false,
            goaway: None,
            reason: Reason::NO_ERROR,
        }
    }

    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>
    ) -> Poll<Option<ProtResult<Reason>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if let Some(frame) = self.goaway.take() {
            if !codec.poll_ready(cx)?.is_ready() {
                self.goaway = Some(frame);
                return Poll::Pending;
            }

            let reason = frame.reason();
            codec.send_frame(Frame::GoAway(frame))?;
            return Poll::Ready(Some(Ok(reason)));
        } else if self.is_close_now() {
            return match self.goaway.as_ref().map(|going_away| going_away.reason()) {
                Some(reason) => Poll::Ready(Some(Ok(reason))),
                None => Poll::Ready(None),
            };
        }
        Poll::Ready(None)
    }

    pub fn go_away_now(&mut self, frame: GoAway) {
        self.close_now = true;
        self.reason = frame.reason();
        self.goaway = Some(frame);
    }

    pub fn is_close_now(&self) -> bool {
        self.close_now
    }

    pub fn reason(&self) -> &Reason {
        &self.reason
    }
}
