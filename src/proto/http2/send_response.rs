use std::sync::{Arc, Mutex, mpsc::Sender};
use std::sync::mpsc::{channel, Receiver};
use rbtree::RBTree;
use webparse::BinaryMut;
use webparse::{
    http::http2::{frame::{Frame, Priority, PriorityFrame, StreamIdentifier, FrameHeader, Kind, Flag, Headers, Data}, Decoder},
    Binary, Serialize, Response, Method,
};

use crate::ProtoResult;

use super::SendStream;

#[derive(Debug)]
pub struct SendResponse {
    pub stream_id: StreamIdentifier,
    pub response: Response<Binary>,
    pub encode_header: bool,
    pub encode_body: bool,
    pub is_end_stream: bool,

    pub method: Method,
    pub receiver: Option<Receiver<(bool, Binary)>>,
}

impl SendResponse {
    pub fn new(stream_id: StreamIdentifier, response: Response<Binary>, method: Method, is_end_stream: bool) -> Self {
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
            let flag = if self.is_end_stream { Flag::end_stream() } else { Flag::zero() };
            let header = FrameHeader::new(Kind::Data, flag, self.stream_id);
            let data = Data::new(header, self.response.body().clone());
            result.push(Frame::Data(data));
            self.encode_body = true;
        }
        if let Some(recv) = &self.receiver {
            while let Ok(val) = recv.try_recv() {
                let flag = if val.0 { Flag::end_stream() } else { Flag::zero() };
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
            let (sender, receiver) = channel::<(bool, Binary)>();

            self.receiver = Some(receiver);

            SendStream::new(sender, BinaryMut::new())

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
    pub fn new(stream_id: StreamIdentifier, queue: Arc<Mutex<Vec<SendResponse>>>, method: Method) -> Self {
        SendControl {
            stream_id,
            queue,
            method,
        }
    }

    pub fn send_response(&mut self, res: Response<Binary>, is_end_stream: bool) -> ProtoResult<SendStream> {
        let mut data = self.queue.lock().unwrap();
        let mut response = SendResponse::new(self.stream_id, res, self.method.clone(), is_end_stream);
        let steam = response.create_sendstream();
        data.push(response);
        Ok(steam)
    }
}

unsafe impl Sync for SendControl {

}

unsafe impl Send for SendControl {

}