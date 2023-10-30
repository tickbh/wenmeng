
use tokio::sync::mpsc::{channel, Sender};
use webparse::{
    http::{
        http2::frame::{Frame, Reason, StreamIdentifier},
        request, response,
    },
    Binary, BinaryMut, Request, Buf, Response,
};

use crate::{ProtError, ProtResult};

use crate::RecvStream;

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

    pub fn push(&mut self, frame: Frame<Binary>) -> ProtResult<()> {
        if frame.is_end_headers() {
            self.end_headers = true;
        }
        if frame.is_end_stream() {
            self.end_stream = true;
        }
        if let Some(sender) = &self.sender {
            if let Ok(p) = sender.try_reserve() {
                match frame {
                    Frame::Data(d) => {
                        self.recv_len += d.payload().remaining();
                        p.send((d.is_end_stream(), d.into_payload()));
                        if self.recv_len > self.content_len {
                            return Err(ProtError::Extension("content len must not more"));
                        }
                    }
                    _ => {
                        return Err(ProtError::Extension("must be data frame"));
                    }
                }
            } else {
                self.frames.push(frame);
            }
        } else {
            self.frames.push(frame);
        }
        Ok(())
    }

    pub fn take(&mut self) -> Vec<Frame<Binary>> {
        self.frames.drain(..).collect()
    }

    pub fn build_request(&mut self) -> ProtResult<Request<RecvStream>> {
        let now_frames = self.take();
        let mut builder = request::Request::builder();
        let mut is_nobody = false;
        let mut is_end_stream = false;
        let mut binary = BinaryMut::new();
        for v in now_frames {
            match v {
                Frame::Headers(header) => {
                    is_nobody = header.is_end_stream();
                    is_end_stream = header.is_end_stream();
                    match header.into_request(builder) {
                        Ok(b) => builder = b,
                        Err(e) => return Err(e.into()),
                    }
                },
                Frame::Data(d) => {
                    is_end_stream = d.is_end_stream();
                    binary.put_slice(d.payload().chunk());
                },
                _ => {
                    return Err(ProtError::library_go_away(Reason::PROTOCOL_ERROR));
                }
            }
        }
        let recv = if is_nobody {
            RecvStream::empty()
        } else {
            let (sender, receiver) = channel::<(bool, Binary)>(20);
            self.sender = Some(sender);
            RecvStream::new(receiver, binary, is_end_stream)
        };
        self.content_len = builder.get_body_len() as usize;
        if self.content_len == 0 {
            self.content_len = usize::MAX;
        }
        match builder.body(recv) {
            Err(e) => return Err(e.into()),
            Ok(r) => return Ok(r),
        }
    }


    pub fn build_response(&mut self) -> ProtResult<Response<RecvStream>> {
        let now_frames = self.take();
        let mut builder = response::Response::builder();
        let mut is_nobody = false;
        let mut is_end_stream = false;
        let mut binary = BinaryMut::new();
        for v in now_frames {
            match v {
                Frame::Headers(header) => {
                    is_nobody = header.is_end_stream();
                    is_end_stream = header.is_end_stream();
                    match header.into_response(builder) {
                        Ok(b) => builder = b,
                        Err(e) => return Err(e.into()),
                    }
                },
                Frame::Data(d) => {
                    is_end_stream = d.is_end_stream();
                    binary.put_slice(d.payload().chunk());
                },
                _ => {
                    return Err(ProtError::library_go_away(Reason::PROTOCOL_ERROR));
                }
            }
        }
        let recv = if is_nobody {
            RecvStream::empty()
        } else {
            let (sender, receiver) = channel::<(bool, Binary)>(20);
            self.sender = Some(sender);
            RecvStream::new(receiver, binary, is_end_stream)
        };
        self.content_len = builder.get_body_len() as usize;
        if self.content_len == 0 {
            self.content_len = usize::MAX;
        }
        match builder.body(recv) {
            Err(e) => return Err(e.into()),
            Ok(r) => return Ok(r),
        }
    }
}
