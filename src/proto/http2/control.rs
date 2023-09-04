use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::{ready, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{
    http::http2::frame::{Flag, Frame, Kind, StreamIdentifier},
    Binary, BinaryMut, Request,
};

use crate::ProtoResult;

use super::{codec::Codec, inner_stream::InnerStream, state::StateHandshake, StateSettings};

pub struct Control {
    /// 所有收到的帧, 如果收到Header结束就开始返回request, 后续收到Data再继续返回直至结束,
    /// id为0的帧为控制帧, 需要立即做处理
    recv_frames: HashMap<StreamIdentifier, InnerStream>,

    handshake: StateHandshake,
    setting: StateSettings,
    next_must_be: Option<(Kind, Flag)>,
}

impl Control {
    pub fn new() -> Self {
        Control {
            next_must_be: None,
            recv_frames: HashMap::new(),
            setting: StateSettings::new(),
            handshake: StateHandshake::new_server(),
        }
    }

    pub fn poll_ready<T>(&mut self, cx: &mut Context, codec: &mut Codec<T>) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ready!(codec.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    pub fn pull_request<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<Option<ProtoResult<Request<Binary>>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ready!(self.handshake.pull_handle(cx, codec))?;
        loop {
            ready!(self.setting.pull_handle(cx, codec))?;
            ready!(self.poll_ready(cx, codec))?;
            match ready!(Pin::new(&mut *codec).poll_next(cx)) {
                Some(Ok(frame)) => {
                    let mut bytes = BinaryMut::new();
                    match frame {
                        Frame::Settings(settings) => {
                            self.setting.recv_setting(settings)?;
                        }
                        Frame::Data(_) => todo!(),
                        Frame::Headers(_) => todo!(),
                        Frame::Priority(_) => todo!(),
                        Frame::PushPromise(_) => todo!(),
                        Frame::Ping(_) => todo!(),
                        Frame::GoAway(_) => todo!(),
                        Frame::WindowUpdate(_) => todo!(),
                        Frame::Reset(_) => todo!(),
                    }
                }
                Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                None => return Poll::Ready(None),
            }
        }
        Poll::Pending
    }
}
