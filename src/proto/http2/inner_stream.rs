
use tokio::sync::mpsc::{channel, Sender};
use webparse::{
    http::{
        http2::frame::{Frame, Reason, StreamIdentifier},
        request,
    },
    Binary, BinaryMut, Request, Buf,
};

use crate::{ProtoError, ProtoResult};

use super::RecvStream;

/// 组成帧的基本数据
pub struct InnerStream {
    id: StreamIdentifier,
    frames: Vec<Frame<Binary>>,
    sender: Option<Sender<(bool, Binary)>>,
    content_len: usize,
    recv_len: usize,
    end_headers: bool,
    end_stream: bool,
}

impl InnerStream {
    pub fn new(frame: Frame<Binary>) -> Self {
        InnerStream {
            id: frame.stream_id(),
            frames: vec![frame],
            sender: None,
            content_len: 0,
            recv_len: 0,
            end_headers: false,
            end_stream: false,
        }
    }

    pub fn push(&mut self, frame: Frame<Binary>) -> ProtoResult<()> {
        if frame.is_end_headers() {
            self.end_headers = true;
        }
        if frame.is_end_stream() {
            self.end_stream = true;
        }
        if let Some(sender) = &self.sender {
            match frame {
                Frame::Data(d) => {
                    self.recv_len += d.payload().remaining();
                    if let Err(_e) = sender.try_send((d.is_end_stream(), d.into_payload())) {
                        return Err(ProtoError::Extension("must be data frame"));
                    }
                    if self.recv_len > self.content_len {
                        return Err(ProtoError::Extension("content len must not more"));
                    }
                }
                _ => {
                    return Err(ProtoError::Extension("must be data frame"));
                }
            }
        } else {
            self.frames.push(frame);
        }
        Ok(())
    }

    pub fn take(&mut self) -> Vec<Frame<Binary>> {
        self.frames.drain(..).collect()
    }

    pub fn build_request(&mut self) -> ProtoResult<Request<RecvStream>> {
        let mut now_frames = self.take();
        let mut builder = request::Request::builder();
        for v in now_frames {
            match v {
                Frame::Headers(header) => match header.into_request(builder) {
                    Ok(b) => builder = b,
                    Err(e) => return Err(e.into()),
                },
                _ => {
                    return Err(ProtoError::library_go_away(Reason::PROTOCOL_ERROR));
                }
            }
        }
        let recv = if self.end_stream {
            RecvStream::empty()
        } else {
            let (sender, receiver) = channel::<(bool, Binary)>(20);
            self.sender = Some(sender);
            RecvStream::new(receiver, BinaryMut::new())
        };
        self.content_len = builder.get_body_len();
        match builder.body(recv) {
            Err(e) => return Err(e.into()),
            Ok(r) => return Ok(r),
        }
    }
}
