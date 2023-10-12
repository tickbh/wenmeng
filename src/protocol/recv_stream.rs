use std::{
    fmt::Display,
    future::poll_fn,
    io::{Read, Write},
    pin::Pin,
    task::{ready, Context, Poll},
};

use flate2::{Compression, GzBuilder, write::GzEncoder};
use futures_core::Stream;
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    sync::mpsc::{error::TryRecvError, Receiver},
};
use webparse::{Binary, BinaryMut, Buf, BufMut, Helper, Serialize};

use crate::ProtResult;

#[derive(Debug)]
struct InnerReceiver {
    receiver: Option<Receiver<(bool, Binary)>>,
    file: Option<File>,
    cache_buf: Vec<u8>,
}
impl InnerReceiver {
    pub fn new() -> Self {
        Self {
            receiver: None,
            file: None,
            cache_buf: vec![],
        }
    }

    pub fn new_receiver(receiver: Receiver<(bool, Binary)>) -> Self {
        let mut vec = Vec::with_capacity(20480);
        vec.resize(20480, 0);
        Self {
            receiver: Some(receiver),
            file: None,
            cache_buf: vec,
        }
    }

    pub fn new_file(file: File) -> Self {
        let mut vec = Vec::with_capacity(20480);
        vec.resize(20480, 0);
        Self {
            receiver: None,
            file: Some(file),
            cache_buf: vec,
        }
    }

    pub fn is_none(&self) -> bool {
        self.receiver.is_none() && self.file.is_none()
    }

    // pub fn try_recv(&mut self) -> Option<(bool, Binary)> {
    //     if let Some(receiver) = &mut self.receiver {
    //         match receiver.try_recv() {
    //             Ok(v) => return Some(v),
    //             Err(TryRecvError::Disconnected) => {
    //                 return Some((true, Binary::new()));
    //             }
    //             Err(TryRecvError::Empty) => {
    //                 return None
    //             }
    //         }
    //     }

    //     if let Some(file) = &mut self.file {
    //         let size = poll_fn(|cx| {
    //             let mut buf = ReadBuf::new(&mut self.cache_buf);
    //             Pin::new(file).poll_read(cx, &mut buf)
    //         });

    //         // file.
    //         match file.read(&mut self.cache_buf).await {
    //             Ok(n) => {
    //                 if n < self.cache_buf.len() {
    //                     return Some((true, Binary::from(self.cache_buf[..n].to_vec())))
    //                 } else {
    //                     return Some((false, Binary::from(self.cache_buf[..n].to_vec())))
    //                 }
    //             },
    //             Err(_) => return None,
    //         };
    //     }
    //     None
    // }

    pub async fn recv(&mut self) -> Option<(bool, Binary)> {
        if let Some(receiver) = &mut self.receiver {
            return receiver.recv().await;
        }

        if let Some(file) = &mut self.file {
            match file.read(&mut self.cache_buf).await {
                Ok(n) => {
                    if n < self.cache_buf.len() {
                        return Some((true, Binary::from(self.cache_buf[..n].to_vec())));
                    } else {
                        return Some((false, Binary::from(self.cache_buf[..n].to_vec())));
                    }
                }
                Err(_) => return None,
            };
        }
        None
    }

    fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<(bool, Binary)>> {
        if let Some(receiver) = &mut self.receiver {
            return receiver.poll_recv(cx);
        }

        if let Some(file) = &mut self.file {
            let size = {
                let mut buf = ReadBuf::new(&mut self.cache_buf);
                match ready!(Pin::new(file).poll_read(cx, &mut buf)) {
                    Ok(_) => buf.filled().len(),
                    Err(_) => return Poll::Ready(None),
                }
            };
            let is_end = size < self.cache_buf.len();
            return Poll::Ready(Some((
                is_end,
                Binary::from(self.cache_buf[..size].to_vec()),
            )));
        }

        return Poll::Ready(None);
    }
}

#[derive(Debug)]
struct InnerCompress {
    gz: Option<GzEncoder<BinaryMut>>,
}

impl InnerCompress {
    pub fn new() -> Self {
        Self { gz: None }
    }

    pub fn open_gz(&mut self) {
        if self.gz.is_none() {
            self.gz = Some(GzEncoder::new(BinaryMut::new(), Compression::default()));
        }
    }
}

#[derive(Debug)]
pub struct RecvStream {
    receiver: InnerReceiver,
    binary: Option<Binary>,
    binary_mut: Option<BinaryMut>,
    compress_method: u8,
    compress: InnerCompress,
    is_end: bool,
}

impl Default for RecvStream {
    fn default() -> Self {
        Self {
            receiver: InnerReceiver::new(),
            binary: Default::default(),
            binary_mut: Default::default(),
            compress_method: Default::default(),
            compress: InnerCompress::new(),
            is_end: true,
        }
    }
}

impl RecvStream {
    pub fn empty() -> RecvStream {
        Default::default()
    }

    pub fn only(binary: Binary) -> RecvStream {
        RecvStream {
            binary: Some(binary),
            ..Default::default()
        }
    }

    pub fn new(receiver: Receiver<(bool, Binary)>, binary: BinaryMut, is_end: bool) -> RecvStream {
        RecvStream {
            receiver: InnerReceiver::new_receiver(receiver),
            binary_mut: Some(binary),
            is_end,
            ..Default::default()
        }
    }

    pub fn new_file(file: File, binary: BinaryMut, is_end: bool) -> RecvStream {
        RecvStream {
            receiver: InnerReceiver::new_file(file),
            binary_mut: Some(binary),
            is_end,
            ..Default::default()
        }
    }

    pub fn binary(&mut self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = self.binary.take() {
            buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = self.binary_mut.take() {
            buffer.put_slice(bin.chunk());
        }
        buffer.freeze()
    }

    pub fn cache_buffer(&mut self, buf: &[u8]) {
        if self.binary_mut.is_none() {
            self.binary_mut = Some(BinaryMut::new());
        }
        self.binary_mut.as_mut().unwrap().put_slice(buf);
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn set_end(&mut self, end: bool) {
        self.is_end = end
    }

    // pub fn try_recv(&mut self) {
    //     if self.receiver.is_none() {
    //         return;
    //     }
    //     while let Some(v) = self.receiver.try_recv() {
    //         if self.binary_mut.is_none() {
    //             self.binary_mut = Some(BinaryMut::new());
    //         }
    //         self.binary_mut.as_mut().unwrap().put_slice(v.1.chunk());
    //         self.is_end = v.0;
    //         if self.is_end == true {
    //             break;
    //         }
    //     }
    // }

    pub fn read_now(&mut self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = self.binary.take() {
            buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = self.binary_mut.take() {
            buffer.put_slice(bin.chunk());
        }
        return buffer.freeze();
    }

    pub fn copy_now(&self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = &self.binary {
            buffer.put_slice(bin.chunk());
        }
        if let Some(bin) = &self.binary_mut {
            buffer.put_slice(bin.chunk());
        }
        return buffer.freeze();
    }

    pub fn body_len(&self) -> usize {
        let mut len = 0;
        if let Some(bin) = &self.binary {
            len += bin.remaining();
        }
        if let Some(bin) = &self.binary_mut {
            len += bin.remaining();
        }
        return len;
    }

    pub async fn wait_all(&mut self) -> Option<usize> {
        let mut size = 0;
        if self.receiver.is_none() || self.is_end {
            return Some(size);
        }
        while let Some(v) = self.receiver.recv().await {
            if self.binary_mut.is_none() {
                self.binary_mut = Some(BinaryMut::new());
            }
            size += self.binary_mut.as_mut().unwrap().put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
        Some(size)
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
        while let Some(v) = self.receiver.recv().await {
            size += buffer.put_slice(v.1.chunk());
            self.is_end = v.0;
            if self.is_end == true {
                break;
            }
        }
        Some(size)
    }

    fn encode_data<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        buffer: &mut B,
        data: &[u8],
        is_chunked: bool,
    ) -> webparse::WebResult<usize> {
        use std::io::{Read};
        if is_chunked {
            // if data.len() == 0 {
            //     self.compress.open_gz();
            //     let mut gz = self.compress.gz.take().unwrap();
            //     let value = gz.finish().unwrap();
            //     if value.remaining() > 0 {
            //         Helper::encode_chunk_data(buffer, &value);
            //     }
            //     Helper::encode_chunk_data(buffer, data)
            // } else {
            //     self.compress.open_gz();
            //     let gz = self.compress.gz.as_mut().unwrap();
            //     gz.write_all(data).unwrap();
            //     let s = Helper::encode_chunk_data(buffer, &gz.get_mut().chunk());
            //     gz.get_mut().clear();
            //     s
            //     // Ok(1)
            //     // Ok(0)
            // }
            Helper::encode_chunk_data(buffer, data)
        } else {
            Ok(buffer.put_slice(data))
        }
    }

    pub fn poll_encode<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &mut B,
        is_chunked: bool,
    ) -> Poll<webparse::WebResult<usize>> {
        let mut size = 0;
        if let Some(bin) = self.binary.take() {
            if bin.chunk().len() > 0 {
                size += self.encode_data(buffer, bin.chunk(), is_chunked)?;
            }
        }
        if let Some(bin) = self.binary_mut.take() {
            if bin.chunk().len() > 0 {
                size += self.encode_data(buffer, bin.chunk(), is_chunked)?;
            }
        }
        let mut has_encode_end = false;
        if !self.is_end {
            loop {
                match self.receiver.poll_recv(cx) {
                    Poll::Pending => {
                        break;
                    }
                    Poll::Ready(Some((is_end, bin))) => {
                        size += self.encode_data(buffer, bin.chunk(), is_chunked)?;
                        self.is_end = is_end;
                        if bin.remaining() == 0 {
                            has_encode_end = is_end;
                        }
                    }
                    Poll::Ready(None) => {
                        self.is_end = true;
                        break;
                    }
                }
            }
        }
        if !has_encode_end && is_chunked && self.is_end {
            self.encode_data(buffer, &[], is_chunked)?;
        }
        Poll::Ready(Ok(size))
    }
}

impl Stream for RecvStream {
    type Item = ProtResult<Binary>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl AsyncRead for RecvStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut read_size = 0;
        if let Some(bin) = &mut self.binary {
            if bin.remaining() > 0 {
                let len = std::cmp::min(buf.remaining(), bin.remaining());
                buf.put_slice(&bin[..len]);
                read_size += len;
            }
        }
        if let Some(bin) = &mut self.binary_mut {
            if bin.remaining() > 0 {
                let len = std::cmp::min(buf.remaining(), bin.remaining());
                buf.put_slice(&bin[..len]);
                read_size += len;
            }
        }
        if read_size > 0 || self.is_end {
            return Poll::Ready(Ok(()));
        }
        if let Some((is_end, bin)) = ready!(self.receiver.poll_recv(cx)) {
            let len = std::cmp::min(buf.remaining(), bin.remaining());
            buf.put_slice(&bin[..len]);
            self.is_end = is_end;
            if len < bin.remaining() {
                self.cache_buffer(&bin.chunk()[len..])
            }
        }
        return Poll::Ready(Ok(()));
    }
}

impl Serialize for RecvStream {
    fn serialize<B: webparse::Buf + webparse::BufMut>(
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
        // if !self.is_end {
        //     loop {
        //         match self.receiver.try_recv() {
        //             Some((is_end, mut bin)) => {
        //                 size += bin.serialize(buffer)?;
        //                 self.is_end = is_end;
        //             }
        //             None => {
        //                 return Ok(size);
        //             }
        //         }
        //     }
        // }
        Ok(size)
    }
}

unsafe impl Sync for RecvStream {}

unsafe impl Send for RecvStream {}

impl From<()> for RecvStream {
    fn from(_: ()) -> Self {
        RecvStream::empty()
    }
}

impl From<&str> for RecvStream {
    fn from(value: &str) -> Self {
        let bin = Binary::from(value.as_bytes().to_vec());
        RecvStream::only(bin)
    }
}

impl From<Binary> for RecvStream {
    fn from(value: Binary) -> Self {
        RecvStream::only(value)
    }
}

impl From<String> for RecvStream {
    fn from(value: String) -> Self {
        let bin = Binary::from(value.into_bytes().to_vec());
        RecvStream::only(bin)
    }
}

impl From<Vec<u8>> for RecvStream {
    fn from(value: Vec<u8>) -> Self {
        let bin = Binary::from(value);
        RecvStream::only(bin)
    }
}

impl From<RecvStream> for Vec<u8> {
    fn from(mut value: RecvStream) -> Self {
        let bin = value.read_now();
        bin.into_slice_all()
    }
}

impl From<RecvStream> for String {
    fn from(mut value: RecvStream) -> Self {
        let bin = value.read_now();
        let v = bin.into_slice_all();
        String::from_utf8_lossy(&v).to_string()
    }
}

impl Display for RecvStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_end {
            let bin = self.copy_now();
            f.write_str(&String::from_utf8_lossy(bin.chunk()))
        } else {
            let mut f = f.debug_struct("RecvStream");
            f.field("状态", &self.is_end);
            if self.is_end {
                f.field("接收字节数", &self.body_len());
            }
            f.finish()
        }
    }
}
