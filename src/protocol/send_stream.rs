use std::{io::Read};

use futures_core::Stream;
use tokio::sync::mpsc::{Sender, error::TrySendError};
use webparse::{Binary, Serialize};

use crate::ProtResult;

#[derive(Debug)]
pub struct SendStream {
    sender: Option<Sender<(bool, Binary)>>,
    is_end: bool,
}

impl SendStream {
    pub fn empty() -> SendStream {
        SendStream {
            sender: None,
            is_end: true,
        }
    }

    pub fn new(sender: Sender<(bool, Binary)>) -> SendStream {
        SendStream {
            sender: Some(sender),
            is_end: false,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.sender.as_ref().map(|s| s.capacity() > 0).unwrap_or(false)
    }

    /// 返回Some则表示数据发送不成功，需要重新进行投递
    pub fn send_data(&mut self, binary: Binary, is_end_stream: bool) -> Option<(bool, Binary)> {
        if let Some(Err(e)) = self.sender.as_ref().map(|s| {
            s.try_send((is_end_stream, binary))
        }) {
            return Some(match e {
                TrySendError::Closed(v) => v,
                TrySendError::Full(v) => v,
            });
        }
        None
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }
}

impl Stream for SendStream {
    type Item=ProtResult<Binary>;

    fn poll_next(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl Read for SendStream {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

impl Serialize for SendStream {
    fn serialize<B: webparse::Buf+webparse::BufMut>(&mut self, _buffer: &mut B) -> webparse::WebResult<usize> {
        Ok(0)
    }
}

unsafe impl Sync for SendStream {

}

unsafe impl Send for SendStream {
    
}
