use std::{
    task::{ready, Context, Poll}, any::{Any, TypeId},
};

use futures_core::{Future, Stream};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::{Receiver, channel},
};
use webparse::{
    http::http2::frame::{Reason, StreamIdentifier},
    Binary, BinaryMut, Request, Response, Serialize, HeaderName, http2::frame::Settings,
};

use crate::{
    protocol::{ProtError, ProtResult},
    Builder, Initiator, RecvStream,
};

use super::{
    codec::{Codec},
    control::ControlConfig,
    Control,
};

pub struct ClientH2Connection<T> {
    codec: Codec<T>,
    inner: InnerConnection,
}

struct InnerConnection {
    state: State,

    control: Control,

}

#[derive(Debug)]
enum State {
    /// Currently open in a sane state
    Open,

    /// The codec must be flushed
    Closing(Reason, Initiator),

    /// In a closed state
    Closed(Reason, Initiator),
}

unsafe impl<T> Sync for ClientH2Connection<T> {}

unsafe impl<T> Send for ClientH2Connection<T> {}

impl<T> ClientH2Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T, builder: Builder) -> ClientH2Connection<T> {
        let (sender, _receiver) = channel(10);
        ClientH2Connection {
            codec: Codec::new(io),
            inner: InnerConnection {
                state: State::Open,
                control: Control::new(ControlConfig {
                    next_stream_id: 1.into(),
                    // Server does not need to locally initiate any streams
                    initial_max_send_streams: 0,
                    max_send_buffer_size: builder.max_send_buffer_size,
                    reset_stream_duration: builder.reset_stream_duration,
                    reset_stream_max: builder.reset_stream_max,
                    remote_reset_stream_max: builder.pending_accept_reset_stream_max,
                    settings: builder.settings.clone(),
                }, sender),
            },
        }
    }

    pub fn pull_accept(&mut self, _cx: &mut Context<'_>) -> Poll<Option<ProtResult<()>>> {
        Poll::Pending
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Request<RecvStream>>>> {
        self.inner.control.poll_request(cx, &mut self.codec)
    }


    pub fn poll_response(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Response<RecvStream>>>> {
        self.inner.control.poll_response(cx, &mut self.codec)
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<()>> {
        self.inner.control.poll_write(cx, &mut self.codec, false)
    }

    pub async fn handle_request<F, Fut, Res, Req>(
        &mut self,
        mut r: Request<RecvStream>,
        f: &mut F,
    ) -> ProtResult<Option<bool>>
    where
        F: FnMut(Request<Req>) -> Fut,
        Fut: Future<Output = ProtResult<Option<Response<Res>>>>,
        Req: From<RecvStream>,
        Req: Serialize + Any,
        RecvStream: From<Res>,
        Res: Serialize + Any,
    {
        let stream_id: Option<StreamIdentifier> = r.extensions_mut().remove::<StreamIdentifier>();
        let mut content_length = 0;
        if TypeId::of::<Req>() != TypeId::of::<RecvStream>() {
            let _ = r.body_mut().wait_all().await;
            content_length = r.body().body_len();
        }
        match f(r.into_type::<Req>()).await? {
            Some(mut res) => {
                if content_length != 0 && res.get_body_len() == 0 {
                    res.headers_mut().insert(HeaderName::CONTENT_LENGTH, content_length);
                }
                self.send_response(
                    res.into_type(),
                    stream_id.unwrap_or(StreamIdentifier::client_first()),
                )
                .await?;
            }
            None => (),
        }
        return Ok(None);
    }

    pub async fn incoming(&mut self) -> ProtResult<Option<Response<RecvStream>>>
    {
        use futures_util::stream::StreamExt;
        tokio::select! {
            res = self.next() => {
                match res {
                    None => return Ok(None),
                    Some(Err(e)) => return Err(e),
                    Some(Ok(r)) => {
                        return Ok(Some(r))
                    }
                };
            }
        }
    }

    fn handle_poll_result(
        &mut self,
        result: Option<ProtResult<Response<RecvStream>>>,
    ) -> ProtResult<()> {
        match result {
            // 收到空包, 则关闭连接
            None => {
                self.inner.state = State::Closing(Reason::NO_ERROR, Initiator::Library);
                Ok(())
            }
            Some(Err(ProtError::GoAway(debug_data, reason, initiator))) => {
                let e = ProtError::GoAway(debug_data.clone(), reason, initiator);
                tracing::debug!(error = ?e, "Connection::poll; connection error");

                if self.inner.control.last_goaway_reason() == &reason {
                    self.inner.state = State::Closing(reason, initiator);
                    return Ok(());
                }
                self.inner.control.go_away_now_data(reason, debug_data);
                // Reset all active streams
                // self.streams.handle_error(e);
                Ok(())
            }
            Some(Err(e)) => {
                self.inner.state = State::Closing(Reason::NO_ERROR, Initiator::Library);
                return Err(e);
            }
            _ => {
                unreachable!();
            }
        }
    }

    fn take_error(&mut self, ours: Reason, initiator: Initiator) -> ProtResult<()> {
        let (debug_data, theirs) = self
            .inner
            .control
            .error
            .take()
            .as_ref()
            .map_or((Binary::new(), Reason::NO_ERROR), |frame| {
                (frame.debug_data().clone(), frame.reason())
            });

        match (ours, theirs) {
            (Reason::NO_ERROR, Reason::NO_ERROR) => Ok(()),
            (ours, Reason::NO_ERROR) => Err(ProtError::GoAway(Binary::new(), ours, initiator)),
            (_, theirs) => Err(ProtError::GoAway(debug_data, theirs, Initiator::Remote)),
        }
    }

    pub fn set_cache_buf(&mut self, read_buf: BinaryMut, write_buf: BinaryMut) {
        self.codec.set_cache_buf(read_buf, write_buf)
    }

    pub fn set_handshake_status(&mut self, binary: Binary) {
        self.inner.control.set_handshake_status(binary, true)
    }

    pub fn set_setting_status(&mut self, setting: Settings, is_done: bool) {
        self.inner.control.set_setting_status(setting, is_done)
    }
    
    pub fn next_stream_id(&mut self) -> StreamIdentifier {
        self.inner.control.next_stream_id()
    }


    pub async fn send_response(
        &mut self,
        res: Response<RecvStream>,
        stream_id: StreamIdentifier,
    ) -> ProtResult<()> {
        self.inner.control.send_response(res, stream_id).await
    }

    pub async fn send_request(
        &mut self,
        res: Request<RecvStream>,
    ) -> ProtResult<()> {
        self.inner.control.send_request(res).await
    }
}

impl<T> Stream for ClientH2Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<Response<RecvStream>>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.state {
                State::Open => {
                    match self.poll_response(cx) {
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                        Poll::Ready(Some(Ok(v))) => {
                            return Poll::Ready(Some(Ok(v)));
                        }
                        Poll::Ready(v) => {
                            let _ = self.handle_poll_result(v)?;
                            continue;
                        }
                    };
                }
                State::Closing(reason, initiator) => {
                    ready!(self.codec.shutdown(cx))?;
                    self.inner.state = State::Closed(reason, initiator);
                }
                State::Closed(reason, initiator) => {
                    if let Err(e) = self.take_error(reason, initiator) {
                        return Poll::Ready(Some(Err(e)));
                    }
                    return Poll::Ready(None);
                }
            }
        }
    }
}
