use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use futures_util::future::poll_fn;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream, sync::mpsc::{Receiver, Sender},
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
    Control, RecvStream, SendResponse, send_response::SendControl,
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
                control: Control::new(ControlConfig {
                    next_stream_id: 2.into(),
                    // Server does not need to locally initiate any streams
                    initial_max_send_streams: 0,
                    max_send_buffer_size: builder.max_send_buffer_size,
                    reset_stream_duration: builder.reset_stream_duration,
                    reset_stream_max: builder.reset_stream_max,
                    remote_reset_stream_max: builder.pending_accept_reset_stream_max,
                    settings: builder.settings.clone(),
                }, sender),
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

    
    pub fn poll_write(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ProtoResult<()>> {
        println!("poll write!!!!!!!");
        self.inner.control.poll_write(cx, &mut self.codec)
        // loop {
        //     ready!(Pin::new(&mut self.codec).poll_next(cx)?);
        // }
    }

    pub async fn incoming(&mut self) -> Option<ProtoResult<(Request<RecvStream>, SendControl)>> {
        use futures_util::stream::StreamExt;
        let mut receiver = self.inner.receiver.take().unwrap();
        // if let Some(_) = receiver.recv().await {
        //     println!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        // }
        loop {
            tokio::select! {
                _ = receiver.recv() => {
                    let _ = poll_fn(|cx| Poll::Ready(self.poll_write(cx))).await;
                    println!("zzzzzz111111111111111111111111111111111111111");
                    // Pin::new(&self.codec).
                },
                v = self.next() => {
                    self.inner.receiver = Some(receiver);
                    return v;
                }
            }
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
        // Pin::new(&mut self.codec.get_mut()).poll_write(cx, &[1, 2, 3, 3, 3, 4, 4, 5, 1]);
        // println!("write");
        // // self.codec
        // Pin::new(&mut self.codec).poll_next(cx)
        
        println!("aaaaaaa do connect");
        loop {
            match self.poll_request(cx) {
                Poll::Pending => {
                    // ready!(self.poll_write(cx))?;
                    println!("pending");
                    return Poll::Pending;
                }
                Poll::Ready(e) => {
                    return Poll::Ready(e);
                } 
            }
        }
        // let xxx = self.poll_request(cx);
        // println!("connect === {:?} ", xxx.is_pending());
        // xxx
    }
}
