use std::{
    collections::HashMap,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{
    http::http2::frame::{Flag, Frame, Kind, StreamIdentifier},
    Binary, Request,
};

use crate::{ProtoResult};

use super::{codec::Codec, Stream, state::Handshake};

pub struct Control {
    /// 所有收到的帧, 如果收到Header结束就开始返回request, 后续收到Data再继续返回直至结束,
    /// id为0的帧为控制帧, 需要立即做处理
    recv_frames: HashMap<StreamIdentifier, Stream>,

    handshake: Handshake,
    next_must_be: Option<(Kind, Flag)>,
}

impl Control {
    pub fn new() -> Self {
        Control {
            next_must_be: None,
            recv_frames: HashMap::new(),
            handshake: Handshake::new_server(),
        }
    }

    fn pull_request<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: Codec<T>,
    ) -> Poll<Option<ProtoResult<Request<Binary>>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        Poll::Pending
    }
}
