// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2023/10/07 09:41:02

use std::task::Context;

use algorithm::buf::{Binary, BinaryMut, Bt};
use webparse::http::http2::frame::{
    Data, Flag, Frame, FrameHeader, Headers, Kind, StreamIdentifier,
};
use webparse::HeaderMap;

use crate::RecvRequest;

#[derive(Debug)]
pub struct SendRequest {
    pub stream_id: StreamIdentifier,
    pub request: RecvRequest,
    pub encode_header: bool,
    pub encode_body: bool,
    pub is_end_stream: bool,
}

impl SendRequest {
    pub fn new(stream_id: StreamIdentifier, request: RecvRequest, is_end_stream: bool) -> Self {
        SendRequest {
            stream_id,
            request,
            encode_header: false,
            encode_body: false,
            is_end_stream,
        }
    }

    pub fn encode_headers(request: &RecvRequest) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(":method", request.method().as_str().to_string());
        headers.insert(":path", request.path().clone());
        let scheme = request.scheme().as_str().to_string();
        let authority = request.get_connect_url().unwrap_or(String::new());
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
            let _ = self.request.body_mut().poll_encode_write(cx, &mut binary);
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
