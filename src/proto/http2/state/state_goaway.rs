use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::http::http2::frame::{GoAway, Reason};

use crate::{proto::http2::codec::Codec, ProtoResult};

pub struct StateGoAway {
    close_now: bool,
    goaway: Option<GoAway>,
}

impl StateGoAway {
    pub fn new() -> Self {
        StateGoAway {
            close_now: false,
            goaway: None,
        }
    }

    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>
    ) -> Poll<Option<ProtoResult<Reason>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        return Poll::Ready(None);
    }

    pub fn go_away_now(&mut self, frame: GoAway) {
        self.close_now = true;
        self.goaway = Some(frame);
    }

    pub fn is_close_now(&self) -> bool {
        self.close_now
    }
}
