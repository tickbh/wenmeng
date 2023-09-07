use std::{io::Read};

use futures_core::Stream;
use tokio::sync::mpsc::{Sender, error::TrySendError};
use webparse::{Binary, BinaryMut, Serialize};

use crate::ProtoResult;

#[derive(Debug)]
pub struct SendStream {
    sender: Option<Sender<(bool, Binary)>>,
    write_sender: Option<Sender<()>>,
    is_end: bool,
}

impl SendStream {
    pub fn empty() -> SendStream {
        SendStream {
            sender: None,
            write_sender: None,
            is_end: true,
        }
    }

    pub fn new(sender: Sender<(bool, Binary)>, write_sender: Sender<()>) -> SendStream {
        SendStream {
            sender: Some(sender),
            write_sender: Some(write_sender),
            is_end: false,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.sender.as_ref().map(|s| s.capacity() > 0).unwrap_or(false)
    }

    pub fn send_data(&mut self, binary: Binary, is_end_stream: bool) -> Result<(), TrySendError<(bool, Binary)>> {
        if let Some(Err(e)) = self.sender.as_ref().map(|s| {
            println!("capacity == {:?} ", s.capacity());
            s.try_send((is_end_stream, binary))
        }) {
            return Err(e);
        }
        self.write_sender.as_ref().map(|s| {
            s.try_send(()) 
        });
        Ok(())
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
