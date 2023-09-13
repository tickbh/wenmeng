use std::{
    pin::Pin,
    task::{ready, Context, Poll}, any::{Any, TypeId},
};

use futures_core::{Future, Stream};
use futures_util::future::poll_fn;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::mpsc::{Receiver, Sender, channel},
};
use webparse::{
    http::http2::frame::{Frame, Reason, Settings, StreamIdentifier},
    Binary, BinaryMut, Request, Response, Serialize,
};

use crate::{
    protocol::{ProtError, ProtResult},
    Builder, Initiator, RecvStream, Server,
};

use super::{
    codec::{Codec, FramedRead, FramedWrite},
    control::ControlConfig,
    send_response::SendControl,
    Control, SendResponse,
};

pub struct H2Connection<T> {
    codec: Codec<T>,
    inner: InnerConnection,
}

struct InnerConnection {
    state: State,

    control: Control,

    receiver_push: Option<Receiver<(StreamIdentifier, Response<RecvStream>)>>,
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

unsafe impl<T> Sync for H2Connection<T> {}

unsafe impl<T> Send for H2Connection<T> {}

impl<T> H2Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T, builder: Builder) -> H2Connection<T> {
        let (sender, receiver) = channel(10);
        H2Connection {
            codec: Codec::new(io),
            inner: InnerConnection {
                state: State::Open,
                control: Control::new(ControlConfig {
                    next_stream_id: 0.into(),
                    // Server does not need to locally initiate any streams
                    initial_max_send_streams: 0,
                    max_send_buffer_size: builder.max_send_buffer_size,
                    reset_stream_duration: builder.reset_stream_duration,
                    reset_stream_max: builder.reset_stream_max,
                    remote_reset_stream_max: builder.pending_accept_reset_stream_max,
                    settings: builder.settings.clone(),
                }, sender),
                receiver_push: Some(receiver),
            },
        }
    }

    pub fn pull_accept(&mut self, cx: &mut Context<'_>) -> Poll<Option<ProtResult<()>>> {
        Poll::Pending
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Request<RecvStream>>>> {
        self.inner.control.poll_request(cx, &mut self.codec)
        // loop {
        //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
        // }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<()>> {
        println!("poll write!!!!!!!");
        self.inner.control.poll_write(cx, &mut self.codec)
        // loop {
        //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
        // }
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
        if TypeId::of::<Req>() != TypeId::of::<RecvStream>() {
            let _ = r.body_mut().wait_all().await;
        }
        match f(r.into_type::<Req>()).await? {
            Some(res) => {
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

    pub async fn incoming<F, Fut, Res, Req>(&mut self, f: &mut F) -> ProtResult<Option<bool>>
    where
    F: FnMut(Request<Req>) -> Fut,
    Fut: Future<Output = ProtResult<Option<Response<Res>>>>,
    Req: From<RecvStream>,
    Req: Serialize + Any,
    RecvStream: From<Res>,
    Res: Serialize + Any,
    {
        use futures_util::stream::StreamExt;
        let mut receiver = self.inner.receiver_push.take().unwrap();
        tokio::select! {
            res = receiver.recv() => {
                self.inner.receiver_push = Some(receiver);
                if res.is_some() {
                    let res = res.unwrap();
                    let id = self.inner.control.next_server_id();
                    self.inner.control.send_response_may_push(res.1, res.0, Some(id)).await?;
                }
            },
            req = self.next() => {
                self.inner.receiver_push = Some(receiver);
                match req {
                    None => return Ok(Some(true)),
                    Some(Err(e)) => return Err(e),
                    Some(Ok(r)) => {
                        self.handle_request(r, f).await?;
                    }
                };
            }
        }
        // let req = self.next().await;
        // match req {
        //     None => return Ok(Some(true)),
        //     Some(Err(e)) => return Err(e),
        //     Some(Ok(r)) => {
        //         self.handle_request(r, f).await?;
        //     }
        // };
        return Ok(None);
    }

    fn handle_poll_result(
        &mut self,
        result: Option<ProtResult<Request<RecvStream>>>,
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
        self.inner.control.set_handshake_status(binary)
    }

    pub async fn send_response(
        &mut self,
        res: Response<RecvStream>,
        stream_id: StreamIdentifier,
    ) -> ProtResult<()> {
        self.inner.control.send_response(res, stream_id).await
    }
}

impl<T> Stream for H2Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtResult<Request<RecvStream>>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            println!("sssssssssssssssssss +{:?}", self.inner.state);
            match self.inner.state {
                State::Open => {
                    match self.poll_request(cx) {
                        Poll::Pending => {
                            println!("pending");
                            // if self.inner.control.error.is_some() {
                            //     self.inner.control.go_away_now(Reason::NO_ERROR);
                            //     continue;
                            // }
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

                    // Transition the state to error
                    self.inner.state = State::Closed(reason, initiator);
                }
                State::Closed(reason, initiator) => {
                    if let Err(e) = self.take_error(reason, initiator) {
                        return Poll::Ready(Some(Err(e)));
                    }
                    println!("Closed!!!!!");
                    return Poll::Ready(None);
                }
            }
        }
        println!("end!!!!!");
        // let xxx = self.poll_request(cx);
        // println!("connect === {:?} ", xxx.is_pending());
        // xxx
    }
}