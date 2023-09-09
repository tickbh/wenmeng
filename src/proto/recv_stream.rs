use std::io::Read;

use bytes::buf;
use futures_core::Stream;
use tokio::sync::mpsc::Receiver;
use webparse::{Binary, BinaryMut, Buf, Serialize};

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

    pub fn new(receiver: Receiver<(bool, Binary)>, binary: BinaryMut, is_end: bool) -> RecvStream {
        RecvStream {
            receiver: Some(receiver),
            binary,
            is_end,
        }
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn try_recv(&mut self) {
        if self.receiver.is_none() {
            return;
        }
        let receiver = self.receiver.as_mut().unwrap();
        while let Ok(v) = receiver.try_recv() {
            self.binary.put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
    }

    pub async fn read_all(&mut self) -> Option<BinaryMut> {
        if self.is_end {
            return Some(self.binary.clone());
        }
        if self.receiver.is_none() {
            return None;
        }
        let receiver = self.receiver.as_mut().unwrap();
        while let Some(v) = receiver.recv().await {
            self.binary.put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
        Some(self.binary.clone())
    }
}

impl Stream for RecvStream {
    type Item = ProtoResult<Binary>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl Read for RecvStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.try_recv();
        let len = std::cmp::min(buf.len(), self.binary.remaining());
        self.binary.copy_to_slice(&mut buf[..len]);
        Ok(len)
    }
}

impl Serialize for RecvStream {
    fn serialize<B: webparse::Buf + webparse::BufMut + webparse::MarkBuf>(
        &self,
        buffer: &mut B,
    ) -> webparse::WebResult<usize> {
        buffer.put_slice(self.binary.chunk());
        // self.receiver.as_ref().unwrap().recv();
        Ok(0)
    }
}

unsafe impl Sync for RecvStream {}

unsafe impl Send for RecvStream {}
