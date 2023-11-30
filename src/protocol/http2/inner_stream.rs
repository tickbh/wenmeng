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
// Created Date: 2023/09/14 09:42:25

use std::{task::{Context, Poll}, collections::LinkedList};

use tokio::sync::mpsc::{channel};
use tokio_util::sync::PollSender;
use webparse::{
    http::{
        http2::frame::{Frame, Reason, StreamIdentifier},
        request, response,
    },
    Binary, BinaryMut, Buf, Version,
};

use crate::{HeaderHelper, ProtError, ProtResult, RecvResponse, RecvRequest};

use crate::RecvStream;

/// 组成帧的基本数据
pub struct InnerStream {
    id: StreamIdentifier,
    frames: LinkedList<Frame<Binary>>,
    sender: Option<PollSender<(bool, Binary)>>,
    content_len: usize,
    recv_len: usize,
    end_headers: bool,
    end_stream: bool,
    is_builder: bool,
}

impl InnerStream {
    pub fn new(frame: Frame<Binary>) -> Self {
        let id = frame.stream_id();
        let mut frames = LinkedList::new();
        frames.push_back(frame);
        InnerStream {
            id,
            frames,
            sender: None,
            content_len: 0,
            recv_len: 0,
            end_headers: false,
            end_stream: false,
            is_builder: false,
        }
    }

    pub fn is_end(&self) -> bool {
        self.is_builder && self.end_stream && self.frames.is_empty()
    }

    pub fn poll_push(&mut self, frame: Frame<Binary>, cx: &mut Context<'_>) -> ProtResult<bool> {
        
        if frame.is_end_headers() {
            self.end_headers = true;
        }
        if frame.is_end_stream() {
            self.end_stream = true;
        }

        self.frames.push_back(frame);
        self.poll_send(cx)
    }

    pub fn poll_send(&mut self, cx: &mut Context<'_>) -> ProtResult<bool> {
        if !self.is_builder {
            return Ok(false);
        }

        while !self.frames.is_empty() {
            if let Some(sender) = &mut self.sender {
                if let Poll::Ready(Ok(_)) = sender.poll_reserve(cx) {
                    let frame = self.frames.pop_front().unwrap();
                    match frame {
                        Frame::Data(d) => {
                            self.recv_len += d.payload().remaining();
                            let _ = sender.send_item((d.is_end_stream(), d.into_payload()));
                            if self.recv_len > self.content_len {
                                return Err(ProtError::Extension("content len must not more"));
                            }
                        }
                        _ => {
                            return Err(ProtError::Extension("must be data frame"));
                        }
                    }
                } else {
                    return Ok(false);
                }
            }
        }

        return Ok(self.end_stream);
    }

    pub fn build_request(&mut self) -> ProtResult<(bool, RecvRequest)> {
        let mut builder = request::Request::builder();
        let mut is_nobody = false;
        let mut is_end_stream = false;
        let mut binary = BinaryMut::new();
        while !self.frames.is_empty() {
            let v = self.frames.pop_front().unwrap();
            match v {
                Frame::Headers(header) => {
                    is_nobody = header.is_end_stream();
                    is_end_stream = header.is_end_stream();
                    match header.into_request(builder) {
                        Ok(b) => builder = b,
                        Err(e) => return Err(e.into()),
                    }
                }
                Frame::Data(d) => {
                    is_end_stream = d.is_end_stream();
                    binary.put_slice(d.payload().chunk());
                }
                _ => {
                    return Err(ProtError::library_go_away(Reason::PROTOCOL_ERROR));
                }
            }
        }
        self.end_stream = is_end_stream;
        let recv = if is_nobody {
            RecvStream::empty()
        } else {
            let (sender, receiver) = channel::<(bool, Binary)>(20);
            self.sender = Some(PollSender::new(sender));
            RecvStream::new(receiver, binary, is_end_stream)
        };
        self.content_len = builder.get_body_len() as usize;
        if self.content_len == 0 {
            self.content_len = usize::MAX;
        }
        self.is_builder = true;
        match builder.body(recv) {
            Err(e) => return Err(e.into()),
            Ok(r) => return Ok((self.is_end(), r)),
        }
    }

    pub fn build_response(&mut self) -> ProtResult<(bool, RecvResponse)> {
        let mut builder = response::Response::builder().version(Version::Http2);
        let mut is_nobody = false;
        let mut is_end_stream = false;
        let mut binary = BinaryMut::new();
        while !self.frames.is_empty() {
            let v = self.frames.pop_front().unwrap();
            match v {
                Frame::Headers(header) => {
                    is_nobody = header.is_end_stream();
                    is_end_stream = header.is_end_stream();
                    match header.into_response(builder) {
                        Ok(b) => builder = b,
                        Err(e) => return Err(e.into()),
                    }
                }
                Frame::Data(d) => {
                    is_end_stream = d.is_end_stream();
                    binary.put_slice(d.payload().chunk());
                }
                _ => {
                    return Err(ProtError::library_go_away(Reason::PROTOCOL_ERROR));
                }
            }
        }
        let mut recv = if is_nobody {
            RecvStream::empty()
        } else {
            let (sender, receiver) = channel::<(bool, Binary)>(20);
            self.sender = Some(PollSender::new(sender));
            RecvStream::new(receiver, binary, is_end_stream)
        };
        HeaderHelper::process_headers(
            Version::Http2,
            true,
            builder.headers_mut().unwrap(),
            &mut recv,
        )?;
        self.content_len = builder.get_body_len() as usize;
        if self.content_len == 0 {
            self.content_len = usize::MAX;
        }
        self.is_builder = true;
        match builder.body(recv) {
            Err(e) => return Err(e.into()),
            Ok(r) => return Ok((self.is_end(), r)),
        }
    }
}
