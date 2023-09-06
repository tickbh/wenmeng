use std::{io::Read, sync::mpsc::{Receiver, Sender}};

use futures_core::Stream;
use webparse::{Binary, BinaryMut, Serialize};

use crate::ProtoResult;

#[derive(Debug)]
pub struct SendStream {
    sender: Option<Sender<(bool, Binary)>>,
    binary: BinaryMut,
    is_end: bool,
}

impl SendStream {
    pub fn empty() -> SendStream {
        SendStream {
            sender: None,
            binary: BinaryMut::new(),
            is_end: true,
        }
    }

    pub fn new(sender: Sender<(bool, Binary)>, binary: BinaryMut) -> SendStream {
        SendStream {
            sender: Some(sender),
            binary,
            is_end: false,
        }
    }

    pub fn send_data(&mut self, binary: Binary, is_end_stream: bool) {
        println!("aaaaaaaaaaaaaaaaaaa {:?}", self.sender);
        self.sender.as_ref().map(|s| {
            println!("send data!!!!!!!!!!!");
            s.send((is_end_stream, binary)) 
        });
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }
}

impl Stream for SendStream {
    type Item=ProtoResult<Binary>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl Read for SendStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

impl Serialize for SendStream {
    fn serialize<B: webparse::Buf+webparse::BufMut+webparse::MarkBuf>(&self, buffer: &mut B) -> webparse::WebResult<usize> {
        Ok(0)
    }
}

unsafe impl Sync for SendStream {

}

unsafe impl Send for SendStream {
    
}
