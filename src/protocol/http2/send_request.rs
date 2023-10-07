

use webparse::http::http2::frame::PushPromise;

use std::task::Context;
use tokio::sync::mpsc::{Sender};
use webparse::{BinaryMut, Buf, Request, HeaderMap, HeaderValue, HeaderName};
use webparse::{
    http::http2::{
        frame::{
            Data, Flag, Frame, FrameHeader, Headers, Kind,
            StreamIdentifier,
        },
    },
    Binary, Method, Response,
};

use crate::{ProtResult, RecvStream};


#[derive(Debug)]
pub struct SendRequest {
    pub stream_id: StreamIdentifier,
    pub request: Request<RecvStream>,
    pub encode_header: bool,
    pub encode_body: bool,
    pub is_end_stream: bool,
}

impl SendRequest {
    pub fn new(
        stream_id: StreamIdentifier,
        request: Request<RecvStream>,
        is_end_stream: bool,
    ) -> Self {
        SendRequest {
            stream_id,
            request,
            encode_header: false,
            encode_body: false,
            is_end_stream,
        }
    }

    pub fn encode_headers(request: & Request<RecvStream>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(":method", request.method().as_str().to_string());
        headers.insert(":path", request.path().clone());
        let scheme = request.scheme().as_str().to_string();
        let authority = request.get_host().unwrap_or(String::new());
        if !scheme.is_empty() {
            headers.insert(":scheme", scheme);
        }
        if !authority.is_empty() {
            headers.insert(":authority", authority);
        }
        for h in request.headers().iter() {
            if h.0 != "Host" {
                headers.insert(h.0.clone(), h.1.clone());
            }
        }
        for h in headers.iter() {
            println!("{}: {}", h.0, h.1);
        }
        headers
    }

    pub fn encode_frames(&mut self, cx: &mut Context) -> (bool, Vec<Frame<Binary>>) {
        let mut result = vec![];
        if !self.encode_header {
            let mut header = FrameHeader::new(Kind::Headers, Flag::end_headers(), self.stream_id);
            if self.request.method().is_nobody() {
                header.flag.set(Flag::end_stream(), true);
            }
            let fields = Self::encode_headers(&self.request);
            let mut header = Headers::new(header, fields);
            header.set_method(self.request.method().clone());
            result.push(Frame::Headers(header));
            self.encode_header = true;
        }

        if !self.request.body().is_end() || !self.encode_body {
            self.encode_body = true;
            let mut binary = BinaryMut::new();
            let _ = self.request.body_mut().poll_encode(cx, &mut binary);
            if binary.remaining() > 0 {
                self.is_end_stream = self.request.body().is_end();
                let flag = if self.is_end_stream {
                    Flag::end_stream()
                } else {
                    Flag::zero()
                };
                let header = FrameHeader::new(Kind::Data, flag, self.stream_id);
                let data = Data::new(header, binary.freeze());
                result.push(Frame::Data(data));
            }
        }

        (self.is_end_stream, result)
    }

}
