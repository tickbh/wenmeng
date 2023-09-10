use std::{io::Read, time};

use bytes::buf;
use futures_core::Stream;
use tokio::{
    join,
    sync::mpsc::{error::TryRecvError, Receiver},
};
use webparse::{Binary, BinaryMut, Buf, Serialize};

use crate::ProtoResult;

#[derive(Debug)]
pub struct RecvStream {
    receiver: Option<Receiver<(bool, Binary)>>,
    binary: Option<Binary>,
    binary_mut: Option<BinaryMut>,
    is_end: bool,
}

impl RecvStream {
    pub fn empty() -> RecvStream {
        RecvStream {
            receiver: None,
            binary: None,
            binary_mut: None,
            is_end: true,
        }
    }

    pub fn only(binary: Binary) -> RecvStream {
        RecvStream {
            receiver: None,
            binary: Some(binary),
            binary_mut: None,
            is_end: true,
        }
    }

    pub fn new(receiver: Receiver<(bool, Binary)>, binary: BinaryMut, is_end: bool) -> RecvStream {
        RecvStream {
            receiver: Some(receiver),
            binary: None,
            binary_mut: Some(binary),
            is_end,
        }
    }

    pub fn binary(&mut self) -> Binary {
        self.binary.take().unwrap_or(Binary::new())
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
            if self.binary_mut.is_none() {
                self.binary_mut = Some(BinaryMut::new());
            }
            self.binary_mut.as_mut().unwrap().put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
    }

    pub async fn read_all(&mut self, buffer: &mut BinaryMut) -> Option<usize> {
        let mut size = 0;
        if let Some(binary) = &mut self.binary {
            size += buffer.put_slice(binary.chunk());
            binary.advance_all();
        }
        if let Some(binary) = &mut self.binary_mut {
            size += buffer.put_slice(binary.chunk());
            binary.advance_all();
        }
        if self.is_end {
            return Some(size);
        }
        if self.receiver.is_none() {
            return Some(size);
        }
        let receiver = self.receiver.as_mut().unwrap();
        while let Some(v) = receiver.recv().await {
            size += buffer.put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
        Some(size)
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
        let mut read_bytes = 0;
        if let Some(bin) = &mut self.binary {
            if  bin.remaining() > 0 {
                let len = std::cmp::min(buf.len() - read_bytes, bin.remaining());
                read_bytes += bin.copy_to_slice(&mut buf[read_bytes..len]);
            }
        }
        if let Some(bin) = &mut self.binary {
            if  bin.remaining() > 0 {
                let len = std::cmp::min(buf.len() - read_bytes, bin.remaining());
                read_bytes += bin.copy_to_slice(&mut buf[read_bytes..len]);
            }
        }
        Ok(read_bytes)
    }
}

impl Serialize for RecvStream {
    fn serialize<B: webparse::Buf + webparse::BufMut + webparse::MarkBuf>(
        &mut self,
        buffer: &mut B,
    ) -> webparse::WebResult<usize> {
        let mut size = 0;
        if let Some(bin) = self.binary.take() {
            size += buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = self.binary_mut.take() {
            size += buffer.put_slice(bin.chunk());
        }
        // if self.receiver.is_some() && !self.is_end {
        //     loop {
        //         match self.receiver.as_mut().unwrap().try_recv() {
        //             Ok((is_end, mut bin)) => {
        //                 size += bin.serialize(buffer)?;
        //                 self.is_end = is_end;
        //             }
        //             Err(TryRecvError::Disconnected) => {
        //                 self.is_end = true;
        //                 return Ok(size);
        //             }
        //             Err(TryRecvError::Empty) => {
        //                 std::thread::sleep(time::Duration::from_millis(10));
        //             }
        //         }
        //     }
        // }
        Ok(size)
    }
}

unsafe impl Sync for RecvStream {}

unsafe impl Send for RecvStream {}

impl From<Binary> for RecvStream {
    fn from(value: Binary) -> Self {
        RecvStream::only(value)
    }
}

// impl<T> From<T> for RecvStream where T : Serialize {
//     fn from(value: T) -> Self {
//         todo!()
//     }
// }

// impl From<Option<dyn Serialize + ?Sized>> for RecvStream {
//     fn from(value: Option<dyn Serialize + Sized>) -> Self {
//         todo!()
//     }
// }