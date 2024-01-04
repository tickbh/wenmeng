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
            Poll::Ready(None) => todo!(),
            Poll::Ready(Some(Ok(msg))) => {
                println!("msg = {:?}", msg);
                return Poll::Ready(Some(Ok(msg)));
            },
            Poll::Ready(Some(Err(e))) => todo!(),
            Poll::Pending => return Poll::Pending,
        }
        // let mut has_change;
        Poll::Pending
        // loop {
        //     has_change = false;
        //     let is_wait = ready!(self.setting.poll_handle(cx, codec, &mut self.config))?;
        //     // 写入如果pending不直接pending, 等尝试读pending则返回
        //     match self.poll_write(cx, codec, is_wait) {
        //         Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
        //         _ => (),
        //     }

        //     self.poll_recv_frame(cx)?;

        //     match Pin::new(&mut *codec).poll_next(cx) {
        //         Poll::Ready(Some(Ok(frame))) => {
        //             has_change = true;
        //             match &frame {
        //                 Frame::Settings(settings) => {
        //                     self.setting
        //                         .recv_setting(codec, settings.clone(), &mut self.config)?;
        //                 }
        //                 Frame::Data(_) => {
        //                     let _ = self.recv_frame(frame, cx)?;
        //                 }
        //                 Frame::Headers(_) => {
        //                     let _ = self.recv_frame(frame, cx)?;
        //                 }
        //                 Frame::Priority(v) => {
        //                     self.send_frames.priority_recv(v.clone());
        //                 }
        //                 Frame::PushPromise(_) => {}
        //                 Frame::Ping(p) => {
        //                     self.ping_pong.receive(p.clone());
        //                 }
        //                 Frame::GoAway(e) => {
        //                     self.error = Some(e.clone());
        //                 }
        //                 Frame::WindowUpdate(_v) => {
        //                     // self.config.settings.set_initial_window_size(Some(v.size_increment()))
        //                 }
        //                 Frame::Reset(_v) => {}
        //             }
        //         }
        //         Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
        //         Poll::Ready(None) => return Poll::Ready(None),
        //         Poll::Pending => match ready!(self.build_request_frame()?) {
        //             Some(r) => {
        //                 return Poll::Ready(Some(Ok(r)));
        //             }
        //             None => {
        //                 if let Some(e) = &self.error {
        //                     return Poll::Ready(Some(Err(ProtError::library_go_away(e.reason()))));
        //                 } else {
        //                     // 有收到消息, 再处理一次数据, 如ack settings或者goway消息
        //                     if has_change {
        //                         continue;
        //                     } else {
        //                         return Poll::Pending;
        //                     }
        //                 }
        //             }
        //         },
        //     }
        // }
    }
}
