use webparse::{
    http::{http2::frame::{Frame, StreamIdentifier, Reason}, request},
    Binary, Request,
};

use crate::{ProtoResult, ProtoError};

/// 组成帧的基本数据
pub struct InnerStream {
    id: StreamIdentifier,
    frames: Vec<Frame<Binary>>,
    content_len: u32,
    recv_len: u32,
    end_headers: bool,
    end_stream: bool,
}

impl InnerStream {
    pub fn new(frame: Frame<Binary>) -> Self {
        InnerStream {
            id: frame.stream_id(),
            frames: vec![frame],
            content_len: 0,
            recv_len: 0,
            end_headers: false,
            end_stream: false,
        }
    }

    pub fn push(&mut self, frame: Frame<Binary>) {
        if frame.is_end_headers() {
            self.end_headers = true;
        }
        if frame.is_end_stream() {
            self.end_stream = true;
        }
        self.frames.push(frame);
    }

    pub fn take(&mut self) -> Vec<Frame<Binary>> {
        self.frames.drain(..).collect()
    }


    pub fn build_request(&mut self) -> Option<ProtoResult<Request<Binary>>> {
        let mut now_frames = self.take();
        let mut builder = request::Request::builder();
        for v in now_frames {
            match v {
                Frame::Headers(header) => {
                    match header.into_request(builder) {
                        Ok(b) => builder = b,
                        Err(e) => return Some(Err(e.into())),
                    }
                }
                _ => {
                    return Some(Err(ProtoError::library_go_away(Reason::PROTOCOL_ERROR)));
                }
            }
        }
        match builder.body(Binary::new()) {
            Err(e) => return Some(Err(e.into())),
            Ok(r) => return Some(Ok(r))
        }
    }
}
