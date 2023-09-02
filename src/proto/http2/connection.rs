use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use webparse::http::http2::frame::Frame;

use crate::proto::{ProtoError, ProtoResult};

use super::codec::{FramedRead, FramedWrite, Codec};

pub struct Connection<T> {
    codec: Codec<T>,
}


unsafe impl<T> Sync for Connection<T> {
    
}

unsafe impl<T> Send for Connection<T> {
    
}


impl<T> Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Connection<T> {
        Connection {
            codec: Codec::new(io),
        }
    }

    pub fn new_by_codec(codec: Codec<T>) -> Connection<T> {
        Connection {
            codec,
        }
    }
}

impl<T> Stream for Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = ProtoResult<Frame>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.codec).poll_next(cx)
    }
}

// impl Connection {

//     /// Advances the internal state of the connection.
//     pub fn poll(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
//         // XXX(eliza): cloning the span is unfortunately necessary here in
//         // order to placate the borrow checker — `self` is mutably borrowed by
//         // `poll2`, which means that we can't borrow `self.span` to enter it.
//         // The clone is just an atomic ref bump.
//         let span = self.inner.span.clone();
//         let _e = span.enter();
//         let span = tracing::trace_span!("poll");
//         let _e = span.enter();

//         loop {
//             tracing::trace!(connection.state = ?self.inner.state);
//             // TODO: probably clean up this glob of code
//             match self.inner.state {
//                 // When open, continue to poll a frame
//                 State::Open => {
//                     let result = match self.poll2(cx) {
//                         Poll::Ready(result) => result,
//                         // The connection is not ready to make progress
//                         Poll::Pending => {
//                             // Ensure all window updates have been sent.
//                             //
//                             // This will also handle flushing `self.codec`
//                             ready!(self.inner.streams.poll_complete(cx, &mut self.codec))?;

//                             if (self.inner.error.is_some()
//                                 || self.inner.go_away.should_close_on_idle())
//                                 && !self.inner.streams.has_streams()
//                             {
//                                 self.inner.as_dyn().go_away_now(Reason::NO_ERROR);
//                                 continue;
//                             }

//                             return Poll::Pending;
//                         }
//                     };

//                     self.inner.as_dyn().handle_poll2_result(result)?
//                 }
//                 State::Closing(reason, initiator) => {
//                     tracing::trace!("connection closing after flush");
//                     // Flush/shutdown the codec
//                     ready!(self.codec.shutdown(cx))?;

//                     // Transition the state to error
//                     self.inner.state = State::Closed(reason, initiator);
//                 }
//                 State::Closed(reason, initiator) => {
//                     return Poll::Ready(self.take_error(reason, initiator));
//                 }
//             }
//         }
//     }

//     fn poll2(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
//         // This happens outside of the loop to prevent needing to do a clock
//         // check and then comparison of the queue possibly multiple times a
//         // second (and thus, the clock wouldn't have changed enough to matter).
//         self.clear_expired_reset_streams();

//         loop {
//             // First, ensure that the `Connection` is able to receive a frame
//             //
//             // The order here matters:
//             // - poll_go_away may buffer a graceful shutdown GOAWAY frame
//             // - If it has, we've also added a PING to be sent in poll_ready
//             if let Some(reason) = ready!(self.poll_go_away(cx)?) {
//                 if self.inner.go_away.should_close_now() {
//                     if self.inner.go_away.is_user_initiated() {
//                         // A user initiated abrupt shutdown shouldn't return
//                         // the same error back to the user.
//                         return Poll::Ready(Ok(()));
//                     } else {
//                         return Poll::Ready(Err(Error::library_go_away(reason)));
//                     }
//                 }
//                 // Only NO_ERROR should be waiting for idle
//                 debug_assert_eq!(
//                     reason,
//                     Reason::NO_ERROR,
//                     "graceful GOAWAY should be NO_ERROR"
//                 );
//             }
//             ready!(self.poll_ready(cx))?;

//             match self
//                 .inner
//                 .as_dyn()
//                 .recv_frame(ready!(Pin::new(&mut self.codec).poll_next(cx)?))?
//             {
//                 ReceivedFrame::Settings(frame) => {
//                     self.inner.settings.recv_settings(
//                         frame,
//                         &mut self.codec,
//                         &mut self.inner.streams,
//                     )?;
//                 }
//                 ReceivedFrame::Continue => (),
//                 ReceivedFrame::Done => {
//                     return Poll::Ready(Ok(()));
//                 }
//             }
//         }
//     }
// }

// // Extracted part of `Connection` which does not depend on `T`. Reduces the amount of duplicated
// // method instantiations.
// #[derive(Debug)]
// struct ConnectionInner<P, B: Buf = Bytes>
// where
//     P: Peer,
// {
//     /// Tracks the connection level state transitions.
//     state: State,

//     /// An error to report back once complete.
//     ///
//     /// This exists separately from State in order to support
//     /// graceful shutdown.
//     error: Option<frame::GoAway>,

//     /// Pending GOAWAY frames to write.
//     go_away: GoAway,

//     /// Ping/pong handler
//     ping_pong: PingPong,

//     /// Connection settings
//     settings: Settings,

//     /// Stream state handler
//     streams: Streams<B, P>,

//     /// A `tracing` span tracking the lifetime of the connection.
//     span: tracing::Span,

//     /// Client or server
//     _phantom: PhantomData<P>,
// }
