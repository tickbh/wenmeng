use std::{io::Read, sync::mpsc::Receiver};

use futures_core::Stream;
use webparse::{Binary, BinaryMut, Serialize};

use crate::ProtoResult;

#[derive(Debug)]
pub struct RecvStream {
    receiver: Option<Receiver<(bool, Binary)>>,
    binary: BinaryMut,
    is_end: bool,
}

impl RecvStream {
    pub fn empty() -> RecvStream {
        RecvStream {
            receiver: None,
            binary: BinaryMut::new(),
            is_end: true,
        }
    }

    pub fn new(receiver: Receiver<(bool, Binary)>, binary: BinaryMut) -> RecvStream {
        RecvStream {
            receiver: Some(receiver),
            binary,
            is_end: false,
        }
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }
}

impl Stream for RecvStream {
    type Item=ProtoResult<Binary>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl Read for RecvStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

impl Serialize for RecvStream {
    fn serialize<B: webparse::Buf+webparse::BufMut+webparse::MarkBuf>(&self, buffer: &mut B) -> webparse::WebResult<usize> {
        Ok(0)
    }
}

unsafe impl Sync for RecvStream {

}

unsafe impl Send for RecvStream {
    
}
