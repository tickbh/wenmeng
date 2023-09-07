use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use futures_util::future::poll_fn;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};
use webparse::{
    http::http2::frame::{Frame, Reason, Settings},
    Binary, Request,
};

use crate::{
    proto::{ProtoError, ProtoResult},
    Builder, Initiator,
};

use super::{
    codec::{Codec, FramedRead, FramedWrite},
    control::ControlConfig,
    send_response::SendControl,
    Control, RecvStream, SendResponse,
};

pub struct Connection<T> {
    codec: Codec<T>,
    inner: InnerConnection,
}

struct InnerConnection {
    state: State,

    control: Control,



    receiver: Option<Receiver<()>>,
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

unsafe impl<T> Sync for Connection<T> {}

unsafe impl<T> Send for Connection<T> {}

impl<T> Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T, builder: Builder) -> Connection<T> {
        let (sender, receiver) = tokio::sync::mpsc::channel::<()>(1);
        Connection {
            codec: Codec::new(io),
            inner: InnerConnection {
                state: State::Open,
                control: Control::new(
                    ControlConfig {
                        next_stream_id: 2.into(),
                        // Server does not need to locally initiate any streams
                        initial_max_send_streams: 0,
                        max_send_buffer_size: builder.max_send_buffer_size,
                        reset_stream_duration: builder.reset_stream_duration,
                        reset_stream_max: builder.reset_stream_max,
                        remote_reset_stream_max: builder.pending_accept_reset_stream_max,
                        settings: builder.settings.clone(),
                    },
                    sender,
                ),
                receiver: Some(receiver),
            },
        }
    }

    pub fn pull_accept(&mut self, cx: &mut Context<'_>) -> Poll<Option<ProtoResult<()>>> {
        Poll::Pending
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtoResult<(Request<RecvStream>, SendControl)>>> {
        self.inner.control.poll_request(cx, &mut self.codec)
        // loop {
        //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
        // }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtoResult<()>> {
        println!("poll write!!!!!!!");
        self.inner.control.poll_write(cx, &mut self.codec)
        // loop {
        //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
        // }
    }

    pub async fn incoming(&mut self) -> Option<ProtoResult<(Request<RecvStream>, SendControl)>> {
        use futures_util::stream::StreamExt;
        let mut receiver = self.inner.receiver.take().unwrap();
        loop {
            tokio::select! {
                _ = receiver.recv() => {
                    let _ = poll_fn(|cx| Poll::Ready(self.poll_write(cx))).await;
                },
                v = self.next() => {
                    self.inner.receiver = Some(receiver);
                    return v;
                }
            }
        }
    }

    fn handle_poll_result(&mut self, result: Option<ProtoResult<(Request<RecvStream>, SendControl)>>) -> ProtoResult<()> {
        match result {
            // 收到空包, 则关闭连接
            None => {
                self.inner.state = State::Closing(Reason::NO_ERROR, Initiator::Library);
                Ok(())
            }
            Some(Err(ProtoError::GoAway(debug_data, reason, initiator))) => {
                let e = ProtoError::GoAway(debug_data.clone(), reason, initiator);
                tracing::debug!(error = ?e, "Connection::poll; connection error");

                if self.inner.control.last_goaway_reason() == &reason {
                    self.inner.state = State::Closing(reason, initiator);
                    return Ok(())
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
            // // Attempting to read a frame resulted in a stream level error.
            // // This is handled by resetting the frame then trying to read
            // // another frame.
            // Err(Error::Reset(id, reason, initiator)) => {
            //     debug_assert_eq!(initiator, Initiator::Library);
            //     tracing::trace!(?id, ?reason, "stream error");
            //     self.streams.send_reset(id, reason);
            //     Ok(())
            // }
            // // Attempting to read a frame resulted in an I/O error. All
            // // active streams must be reset.
            // //
            // // TODO: Are I/O errors recoverable?
            // Err(Error::Io(e, inner)) => {
            //     tracing::debug!(error = ?e, "Connection::poll; IO error");
            //     let e = Error::Io(e, inner);

            //     // Reset all active streams
            //     self.streams.handle_error(e.clone());

            //     // Return the error
            //     Err(e)
            // }
        }
    }
    
    fn take_error(&mut self, ours: Reason, initiator: Initiator) -> ProtoResult<()> {
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
            (ours, Reason::NO_ERROR) => Err(ProtoError::GoAway(Binary::new(), ours, initiator)),
            (_, theirs) => Err(ProtoError::GoAway(debug_data, theirs, Initiator::Remote)),
        }
    }
}

impl<T> Stream for Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtoResult<(Request<RecvStream>, SendControl)>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {

        loop {
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
                },
                State::Closing(reason, initiator) => {
                    ready!(self.codec.shutdown(cx))?;

                    // Transition the state to error
                    self.inner.state = State::Closed(reason, initiator);
                },
                State::Closed(reason, initiator) =>  {
                    if let Err(e) = self.take_error(reason, initiator) {
                        return Poll::Ready(Some(Err(e)));
                    }
                    return Poll::Ready(None);
                },
            }
            
        }
        // let xxx = self.poll_request(cx);
        // println!("connect === {:?} ", xxx.is_pending());
        // xxx
    }

    
}
