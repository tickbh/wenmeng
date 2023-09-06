use std::sync::{Arc, Mutex};

use rbtree::RBTree;
use webparse::{
    http::http2::{frame::{Frame, Priority, PriorityFrame, StreamIdentifier, FrameHeader, Kind, Flag, Headers, Data}, Decoder},
    Binary, Serialize, Response,
};

use crate::ProtoResult;

#[derive(Debug)]
pub struct SendResponse {
    pub stream_id: StreamIdentifier,
    pub response: Response<Binary>,
    pub encode_header: bool,
}

impl SendResponse {
    pub fn new(stream_id: StreamIdentifier, response: Response<Binary>) -> Self {
        SendResponse {
            stream_id,
            response,
            encode_header: false,
        }
    }

    pub fn encode_frames(&mut self) -> (bool, Vec<Frame<Binary>>) {
        let mut result = vec![];
        if !self.encode_header {
            let header = FrameHeader::new(Kind::Headers, Flag::end_headers(), self.stream_id);
            let mut fields = self.response.headers().clone();
            let val = self.response.status().as_str();
            fields.insert(":status", val);
            result.push(Frame::Headers(Headers::new(header, fields)));
            self.encode_header = true;
        }
        let header = FrameHeader::new(Kind::Data, Flag::end_stream(), self.stream_id);
        let data = Data::new(header, self.response.body().clone());
        result.push(Frame::Data(data));
        (true, result)
    }
}

#[derive(Debug, Clone)]
pub struct SendControl {
    pub stream_id: StreamIdentifier,
    pub queue: Arc<Mutex<Vec<SendResponse>>>,
}

impl SendControl {
    pub fn new(stream_id: StreamIdentifier, queue: Arc<Mutex<Vec<SendResponse>>>) -> Self {
        SendControl {
            stream_id,
            queue,
        }
    }

    pub fn send_response(&mut self, res: Response<Binary>) -> ProtoResult<()> {
        let mut data = self.queue.lock().unwrap();
        data.push(SendResponse::new(self.stream_id, res));
        Ok(())
    }
}

unsafe impl Sync for SendControl {

}

unsafe impl Send for SendControl {

}