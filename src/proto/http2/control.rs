use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll}, time::Duration,
};

use futures_core::{ready, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{
    http::{http2::frame::{Flag, Frame, Kind, StreamIdentifier, Settings}, request},
    Binary, BinaryMut, Request,
};

use crate::ProtoResult;

use super::{codec::Codec, inner_stream::InnerStream, state::StateHandshake, StateSettings, WindowSize};


#[derive(Debug, Clone)]
pub struct ControlConfig {
    pub next_stream_id: StreamIdentifier,
    pub initial_max_send_streams: usize,
    pub max_send_buffer_size: usize,
    pub reset_stream_duration: Duration,
    pub reset_stream_max: usize,
    pub remote_reset_stream_max: usize,
    pub settings: Settings,
}

impl ControlConfig {
    pub fn apply_remote_settings(&mut self, settings: &Settings) {

    }
}

pub struct Control {
    /// 所有收到的帧, 如果收到Header结束就开始返回request, 后续收到Data再继续返回直至结束,
    /// id为0的帧为控制帧, 需要立即做处理
    recv_frames: HashMap<StreamIdentifier, InnerStream>,

    handshake: StateHandshake,
    setting: StateSettings,

    config: ControlConfig,
    next_must_be: Option<(Kind, Flag)>,
}

impl Control {
    pub fn new(config: ControlConfig) -> Self {
        Control {
            next_must_be: None,
            recv_frames: HashMap::new(),
            setting: StateSettings::new(config.settings.clone()),
            handshake: StateHandshake::new_server(),
            config,
        }
    }

    pub fn poll_ready<T>(&mut self, cx: &mut Context, codec: &mut Codec<T>) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ready!(codec.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    pub fn poll_request<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<Option<ProtoResult<Request<Binary>>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ready!(self.handshake.poll_handle(cx, codec))?;
        loop {
            ready!(self.setting.poll_handle(cx, codec, &mut self.config))?;
            ready!(self.poll_ready(cx, codec))?;
            match ready!(Pin::new(&mut *codec).poll_next(cx)) {
                Some(Ok(frame)) => {
                    
                    let mut bytes = BinaryMut::new();
                    match frame {
                        Frame::Settings(settings) => {
                            self.setting.recv_setting(codec, settings, &mut self.config)?;
                        }
                        Frame::Data(_) => {
                        },
                        Frame::Headers(header) => {
                            let mut stream_id = header.stream_id();
                            let mut builder = header.into_request()?;
                            let request = builder.body(Binary::new())?;
                            return Poll::Ready(Some(Ok(request)));
                        },
                        Frame::Priority(_) => {

                        },
                        Frame::PushPromise(_) => {

                        },
                        Frame::Ping(_) => {

                        },
                        Frame::GoAway(_) => {

                        },
                        Frame::WindowUpdate(_) => {

                        },
                        Frame::Reset(_) => {
                            
                        },
                    }
                }
                Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                None => return Poll::Ready(None),
            }
        }
    }

    pub fn build_request(&mut self, frames: &Vec<Frame<Binary>>) -> Option<ProtoResult<Request<Binary>>> {

        None
    }

    pub fn recv_frame(&mut self, frame: Frame<Binary>) -> Option<ProtoResult<Request<Binary>>> {
        let stream_id = frame.stream_id();
        if stream_id.is_zero() {
            return None;
        }

        let is_end_headers = frame.is_end_headers();
        let is_end_stream = frame.is_end_stream();
        
        if !self.recv_frames.contains_key(&stream_id) {
            self.recv_frames.insert(stream_id, InnerStream::new(frame));
        }
        None
    } 
}
