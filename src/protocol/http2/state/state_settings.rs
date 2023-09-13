use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::http::http2::frame::{Frame, Reason, Settings};

use crate::{
    protocol::http2::{codec::Codec, control::ControlConfig, Control},
    ProtError, ProtResult,
};

pub struct StateSettings {
    state: LocalState,
    remote: Option<Settings>,
}

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

    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
        config: &mut ControlConfig,
    ) -> Poll<ProtResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        match &self.state {
            LocalState::Send(settings) => {
                codec.send_frame(Frame::Settings(settings.clone()))?;
                self.state = LocalState::WaitAck(settings.clone());
                return Poll::Ready(Ok(()));
            }
            LocalState::WaitAck(_) => {}
            LocalState::Done => (),
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
        return Poll::Ready(Ok(()));
    }

    pub fn recv_setting<T>(
        &mut self,
        codec: &mut Codec<T>,
        setting: Settings,
        config: &mut ControlConfig,
    ) -> ProtResult<()>
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
            Ok(())
        } else {
            self.remote = Some(setting);
            Ok(())
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
