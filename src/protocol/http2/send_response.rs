

use webparse::http::http2::frame::PushPromise;

use std::task::Context;
use tokio::sync::mpsc::{Sender};
use webparse::{BinaryMut, Buf, HeaderMap, HeaderValue, HeaderName};
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
pub struct SendResponse {
    pub stream_id: StreamIdentifier,
    pub push_id: Option<StreamIdentifier>,
    pub response: Response<RecvStream>,
    pub encode_header: bool,
    pub encode_body: bool,
    pub is_end_stream: bool,

    pub method: Method,
}

impl SendResponse {
    pub fn new(
        stream_id: StreamIdentifier,
        push_id: Option<StreamIdentifier>,
        response: Response<RecvStream>,
        method: Method,
        is_end_stream: bool,
    ) -> Self {
        SendResponse {
            stream_id,
            push_id,
            response,
            encode_header: false,
            encode_body: false,
            is_end_stream,
            method,
        }
    }

    pub fn encode_headers(response: & Response<RecvStream>) -> (HeaderMap, bool) {
        let mut headers = HeaderMap::new();
        let mut is_end = false;
        headers.insert(":status", HeaderValue::from_static(response.status().as_str()));
        for h in response.headers().iter() {
            // if h.0 != HeaderName::TRANSFER_ENCODING {
            //     headers.insert(h.0.clone(), h.1.clone());
            // }
            headers.insert(h.0.clone(), h.1.clone());
            if h.0 == HeaderName::CONTENT_LENGTH && TryInto::<isize>::try_into(&h.1).unwrap_or(0) == 0{
                is_end = true;
            }
        }
        (headers, is_end)
    }

    pub fn encode_frames(&mut self, cx: &mut Context) -> (bool, Vec<Frame<Binary>>) {
        let mut result = vec![];
        if !self.encode_header {
            if let Some(push_id) = &self.push_id {
                let header = FrameHeader::new(Kind::PushPromise, Flag::end_headers(), self.stream_id);
                let (fields, is_end) = Self::encode_headers(&self.response);
                
                let mut push = PushPromise::new(header, push_id.clone(), fields);
                if is_end {
                    push.flags_mut().set_end_stream();
                }
                push.set_status(self.response.status());
                result.push(Frame::PushPromise(push));
                self.stream_id = push_id.clone();
                self.encode_header = true;
            } else {
                let header = FrameHeader::new(Kind::Headers, Flag::end_headers(), self.stream_id);
                let (fields, is_end) = Self::encode_headers(&self.response);
                let mut header = Headers::new(header, fields);
                if is_end {
                    header.flags_mut().set_end_stream();
                }
                header.set_status(self.response.status());
                result.push(Frame::Headers(header));
                self.encode_header = true;
            }
        }

        if !self.response.body().is_end() || !self.encode_body {
            self.encode_body = true;
            let mut binary = BinaryMut::new();
            let _ = self.response.body_mut().poll_encode(cx, &mut binary);
            if binary.remaining() > 0 {
                self.is_end_stream = self.response.body().is_end();
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

#[derive(Debug, Clone)]
pub struct SendControl {
    pub stream_id: StreamIdentifier,
    pub sender: Sender<(StreamIdentifier, Response<RecvStream>)>,
    pub method: Method,
}

impl SendControl {
    pub fn new(
        stream_id: StreamIdentifier,
        sender: Sender<(StreamIdentifier, Response<RecvStream>)>,
        method: Method,
    ) -> Self {
        SendControl {
            stream_id,
            sender,
            method,
        }
    }

    pub async fn send_response(
        &mut self,
        res: Response<RecvStream>
    ) -> ProtResult<()> {
        let _ = self.sender.send((self.stream_id, res)).await;
        Ok(())
    }

}

unsafe impl Sync for SendControl {}

unsafe impl Send for SendControl {}
