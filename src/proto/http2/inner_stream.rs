use webparse::{
    http::http2::frame::{Frame, StreamIdentifier},
    Binary,
};

/// 组成帧的基本数据
pub struct InnerStream {
    id: StreamIdentifier,
    frames: Vec<Frame<Binary>>,
    content_len: u32,
    recv_len: u32,
}

impl InnerStream {
    pub fn new(frame: Frame<Binary>) -> Self {
        InnerStream {
            id: frame.stream_id(),
            frames: vec![frame],
            content_len: 0,
            recv_len: 0,
        }
    }
}
