use std::{sync::Arc, task::{Context, Poll}};

use rbtree::RBTree;
use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{
    http::http2::frame::{Frame, Priority, PriorityFrame, StreamIdentifier},
    Binary, Serialize, Response,
};

use crate::ProtoResult;

use super::codec::Codec;

#[derive(Debug)]
pub struct PriorityQueue {
    pub send_queue: RBTree<PriorityFrame<Binary>, ()>,
}

impl PriorityQueue {
    pub fn new() -> Self {
        PriorityQueue {
            send_queue: RBTree::new(),
        }
    }

    pub fn send_frames(&mut self, stream_id: StreamIdentifier, vec: Vec<Frame<Binary>>) -> ProtoResult<()> {
        for v in vec {
            self.send_queue.insert(PriorityFrame::new(v), ());
        }
        Ok(())
    }



    pub fn poll_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<Option<ProtoResult<()>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        loop {
            if !codec.poll_ready(cx)?.is_ready() || self.send_queue.is_empty() {
                return Poll::Ready(None);
            }
            let first = self.send_queue.pop_first().unwrap();
            codec.send_frame(first.0.frame)?;
        }

        return Poll::Ready(None);
    }

}

unsafe impl Sync for PriorityQueue {

}

unsafe impl Send for PriorityQueue {

}