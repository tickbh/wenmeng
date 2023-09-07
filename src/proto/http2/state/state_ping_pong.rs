use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::http::http2::frame::Ping;

use crate::{proto::http2::codec::Codec, ProtoResult};

pub struct StatePingPong {
    ping: Option<Ping>,
}

impl StatePingPong {
    pub fn new() -> Self {
        StatePingPong { ping: None }
    }

    
    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>
    ) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        return Poll::Ready(Ok(()));
    }
}
