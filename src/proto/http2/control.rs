use std::{
    collections::HashMap,
    task::{Context, Poll}, pin::Pin,
};

use futures_core::{ready, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{
    http::http2::frame::{Flag, Frame, Kind, StreamIdentifier},
    Binary, Request,
};

use crate::{ProtoResult};

use super::{codec::Codec, state::StateHandshake, inner_stream::InnerStream};

pub struct Control {
    /// 所有收到的帧, 如果收到Header结束就开始返回request, 后续收到Data再继续返回直至结束,
    /// id为0的帧为控制帧, 需要立即做处理
    recv_frames: HashMap<StreamIdentifier, InnerStream>,

    handshake: StateHandshake,
    next_must_be: Option<(Kind, Flag)>,
}

impl Control {
    pub fn new() -> Self {
        Control {
            next_must_be: None,
            recv_frames: HashMap::new(),
            handshake: StateHandshake::new_server(),
        }
    }

    fn pull_request<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<Option<ProtoResult<Request<Binary>>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ready!(self.handshake.pull_handle(cx, codec))?;
        match ready!(Pin::new(codec).poll_next(cx)) {
            Some(Ok(frame)) => {
                match frame {
                    Frame::Settings(settings) => {

                    },
                    Frame::Data(_) => todo!(),
                    Frame::Headers(_) => todo!(),
                    Frame::Priority(_) => todo!(),
                    Frame::PushPromise(_) => todo!(),
                    Frame::Ping(_) => todo!(),
                    Frame::GoAway(_) => todo!(),
                    Frame::WindowUpdate(_) => todo!(),
                    Frame::Reset(_) => todo!(),
                }
            },
            Some(Err(e)) => return Poll::Ready(Some(Err(e))),
            None => return Poll::Ready(None),
        }
        Poll::Pending
    }
}
