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

        println!("aaaaaaa do connect");
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
                        Poll::Ready(e) => {
                            return Poll::Ready(e);
                        }
                    }
                },
                State::Closing(reason, initiator) => {
                    // ready!(self.codec.shutdown(cx))?;

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
