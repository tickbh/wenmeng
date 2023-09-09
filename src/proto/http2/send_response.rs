use rbtree::RBTree;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use webparse::BinaryMut;
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
    pub write_sender: Sender<()>,
}

impl SendResponse {
    pub fn new(
        stream_id: StreamIdentifier,
        response: Response<RecvStream>,
        method: Method,
        is_end_stream: bool,
        write_sender: Sender<()>,
    ) -> Self {
        SendResponse {
            stream_id,
            response,
            encode_header: false,
            encode_body: false,
            is_end_stream,
            method,
            receiver: None,
            write_sender,
        }
    }

    pub fn encode_frames(&mut self) -> (bool, Vec<Frame<Binary>>) {
        let mut result = vec![];
        if !self.encode_header {
            let header = FrameHeader::new(Kind::Headers, Flag::end_headers(), self.stream_id);
            let fields = self.response.headers().clone();
            let mut header = Headers::new(header, fields);
            header.set_status(self.response.status());
            result.push(Frame::Headers(header));
            self.encode_header = true;
        }
        if !self.method.res_nobody() && !self.encode_body {
            let flag = if self.is_end_stream {
                Flag::end_stream()
            } else {
                Flag::zero()
            };
            let header = FrameHeader::new(Kind::Data, flag, self.stream_id);
            let data = Data::new(header, self.response.body_mut().binary());
            result.push(Frame::Data(data));
            self.encode_body = true;
        }
        if let Some(recv) = &mut self.receiver {
            while let Ok(val) = recv.try_recv() {
                let flag = if val.0 {
                    Flag::end_stream()
                } else {
                    Flag::zero()
                };
                self.is_end_stream = val.0;
                let header = FrameHeader::new(Kind::Data, flag, self.stream_id);
                let data = Data::new(header, val.1);
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
            SendStream::new(sender, self.write_sender.clone())
        }
    }
}

#[derive(Debug, Clone)]
pub struct SendControl {
    pub stream_id: StreamIdentifier,
    pub queue: Arc<Mutex<Vec<SendResponse>>>,
    pub method: Method,
    pub write_sender: Sender<()>,
}

impl SendControl {
    pub fn new(
        stream_id: StreamIdentifier,
        queue: Arc<Mutex<Vec<SendResponse>>>,
        method: Method,
        write_sender: Sender<()>,
    ) -> Self {
        SendControl {
            stream_id,
            queue,
            method,
            write_sender,
        }
    }

    pub fn send_response(
        &mut self,
        res: Response<RecvStream>,
        is_end_stream: bool,
    ) -> ProtoResult<SendStream> {
        let mut data = self.queue.lock().unwrap();
        let mut response = SendResponse::new(
            self.stream_id,
            res,
            self.method.clone(),
            is_end_stream,
            self.write_sender.clone(),
        );
        let steam = response.create_sendstream();
        data.push(response);
        Ok(steam)
    }
}

unsafe impl Sync for SendControl {}

unsafe impl Send for SendControl {}
