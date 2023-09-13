use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::http::http2::frame::{Ping, Frame};

use crate::{proto::http2::codec::Codec, ProtoResult};

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
    ) -> Poll<ProtoResult<()>>
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
