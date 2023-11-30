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
use webparse::http::http2::frame::{Frame, Reason, Settings};

use crate::{
    protocol::http2::{codec::Codec, control::ControlConfig},
    ProtError, ProtResult,
};

pub struct StateSettings {
    state: LocalState,
    remote: Option<Settings>,
}

#[derive(PartialEq, Eq)]
enum LocalState {
    /// 设置发送的settings
    Send(Settings),
    /// 设置等待确认settings
    WaitAck(Settings),
    /// 发送并收到了设置
    Done,
}

impl StateSettings {
    pub fn new(settings: Settings) -> Self {
        StateSettings {
            state: LocalState::Send(settings),
            remote: None,
        }
    }

    pub fn set_settings(&mut self, setting: Settings, is_done: bool) {
        if is_done {
            self.state = LocalState::Done;
        } else {
            self.state = LocalState::Send(setting);
        }
    }

    pub fn set_settings_done(&mut self) {
        self.state = LocalState::Done;
    }


    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
        config: &mut ControlConfig,
    ) -> Poll<ProtResult<bool>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut is_wait = true;
        match &self.state {
            LocalState::Send(settings) => {
                codec.send_frame(Frame::Settings(settings.clone()))?;
                self.state = LocalState::WaitAck(settings.clone());
            }
            LocalState::WaitAck(_) => {}
            LocalState::Done => is_wait = false,
        };

        if let Some(settings) = &self.remote {
            if !codec.poll_ready(cx)?.is_ready() {
                return Poll::Pending;
            }
            let frame = Settings::ack();
            codec.send_frame(Frame::Settings(frame))?;

            config.apply_remote_settings(settings);
            if let Some(val) = settings.header_table_size() {
                codec.set_send_header_table_size(val as usize);
            }

            if let Some(val) = settings.max_frame_size() {
                codec.set_max_send_frame_size(val as usize);
            }
        }

        self.remote = None;
        return Poll::Ready(Ok(is_wait));
    }

    pub fn recv_setting<T>(
        &mut self,
        codec: &mut Codec<T>,
        setting: Settings,
        config: &mut ControlConfig,
    ) -> ProtResult<bool>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if setting.is_ack() {
            match &self.state {
                LocalState::WaitAck(settings) => {
                    config.apply_remote_settings(settings);
                    if let Some(val) = settings.header_table_size() {
                        codec.set_send_header_table_size(val as usize);
                    }

                    if let Some(val) = settings.max_frame_size() {
                        codec.set_max_send_frame_size(val as usize);
                    }
                }
                _ => {
                    return Err(ProtError::library_go_away(Reason::PROTOCOL_ERROR));
                }
            }
            self.state = LocalState::Done;
            Ok(true)
        } else {
            self.remote = Some(setting);
            Ok(self.state == LocalState::Done)
        }
    }

    pub fn send_settings(&mut self, setting: Settings) -> ProtResult<()> {
        assert!(!setting.is_ack());
        match &self.state {
            LocalState::Send(_) | LocalState::WaitAck(_) => Err(ProtError::Extension("")),
            LocalState::Done => {
                self.state = LocalState::Send(setting);
                Ok(())
            }
        }
    }
}
