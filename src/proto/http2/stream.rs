use webparse::{http::http2::frame::{Frame, StreamIdentifier}, Binary};



/// 组成帧的基本数据 
pub struct Stream {
    id: StreamIdentifier,
    frames: Vec<Frame<Binary>>,
    content_len: u32,
    recv_len: u32,
}