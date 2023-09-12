use rbtree::RBTree;
use std::sync::{Arc, Mutex};
use std::task::Context;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use webparse::{BinaryMut, Buf};
use webparse::{
    http::http2::{
        frame::{
            Data, Flag, Frame, FrameHeader, Headers, Kind, Priority, PriorityFrame,
            StreamIdentifier,
        },
        Decoder,
    },
    Binary, Method, Response, Serialize,
};

use crate::{ProtoResult, RecvStream};
use crate::SendStream;

#[derive(Debug)]
pub struct SendResponse {
    pub stream_id: StreamIdentifier,
    pub response: Response<RecvStream>,
    pub encode_header: bool,
    pub encode_body: bool,
    pub is_end_stream: bool,

    pub method: Method,
    pub receiver: Option<Receiver<(bool, Binary)>>,
}

impl SendResponse {
    pub fn new(
        stream_id: StreamIdentifier,
        response: Response<RecvStream>,
        method: Method,
        is_end_stream: bool,
    ) -> Self {
        SendResponse {
            stream_id,
            response,
            encode_header: false,
            encode_body: false,
            is_end_stream,
            method,
            receiver: None,
        }
    }

    pub fn encode_frames(&mut self, cx: &mut Context) -> (bool, Vec<Frame<Binary>>) {
        let mut result = vec![];
        if !self.encode_header {
            let header = FrameHeader::new(Kind::Headers, Flag::end_headers(), self.stream_id);
            let fields = self.response.headers().clone();
            let mut header = Headers::new(header, fields);
            header.set_status(self.response.status());
            result.push(Frame::Headers(header));
            self.encode_header = true;
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

    pub fn create_sendstream(&mut self) -> SendStream {
        if self.method.res_nobody() || self.is_end_stream {
            SendStream::empty()
        } else {
            let (sender, receiver) = channel::<(bool, Binary)>(100);
            self.receiver = Some(receiver);
            SendStream::new(sender)
        }
    }
}

#[derive(Debug, Clone)]
pub struct SendControl {
    pub stream_id: StreamIdentifier,
    pub queue: Arc<Mutex<Vec<SendResponse>>>,
    pub method: Method,
}

impl SendControl {
    pub fn new(
        stream_id: StreamIdentifier,
        queue: Arc<Mutex<Vec<SendResponse>>>,
        method: Method,
    ) -> Self {
        SendControl {
            stream_id,
            queue,
            method,
        }
    }

    pub fn send_response<R>(
        &mut self,
        res: Response<R>,
        is_end_stream: bool,
    ) -> ProtoResult<SendStream> where
    RecvStream: From<R>,
    R: Serialize, {
        let mut data = self.queue.lock().unwrap();
        let mut response = SendResponse::new(
            self.stream_id,
            res.into_type(),
            self.method.clone(),
            is_end_stream,
        );
        let steam = response.create_sendstream();
        data.push(response);
        Ok(steam)
    }
}

unsafe impl Sync for SendControl {}

unsafe impl Send for SendControl {}
