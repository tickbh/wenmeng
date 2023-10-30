use brotli::{CompressorWriter, Decompressor};
use flate2::{
    write::{DeflateEncoder, GzEncoder},
    Compression, read::{GzDecoder, DeflateDecoder},
};

use std::{fmt::Debug, io};
use std::{
    fmt::Display,
    io::{Read, Write},
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    sync::mpsc::Receiver,
};
use webparse::{Binary, BinaryMut, Buf, BufMut, Helper, Serialize, WebResult, WebError};

use crate::{Consts, ProtResult};


fn read_all_data<R: Read>(read_buf: &mut BinaryMut, mut read: R) -> webparse::WebResult<usize> {
    let mut cache_buf = vec![0u8; 4096];
    let mut size = 0;
    loop {
        let s = read.read(&mut cache_buf)?;
        size += s;
        read_buf.put_slice(&cache_buf[..s]);
        if s < cache_buf.len() {
            return Ok(size)
        }
    }
}

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
        let vec = vec![0u8; 4096];
        Self {
            receiver: Some(receiver),
            file: None,
            cache_buf: vec,
        }
    }

    pub fn new_file(file: File) -> Self {
        let vec = vec![0u8; 409600];
        Self {
            receiver: None,
            file: Some(file),
            cache_buf: vec,
        }
    }

    pub fn is_none(&self) -> bool {
        self.receiver.is_none() && self.file.is_none()
    }

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
            println!("file name = {:?}", file);
            let size = {
                let mut buf = ReadBuf::new(&mut self.cache_buf);
                match ready!(Pin::new(file).poll_read(cx, &mut buf)) {
                    Ok(_) => buf.filled().len(),
                    Err(e) => { 
                        log::trace!("读取文件时出错:{:?}", e);
                        return Poll::Ready(None);
                    }
                    
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

struct InnerCompress {
    write_gz: Option<GzEncoder<BinaryMut>>,
    write_br: Option<CompressorWriter<BinaryMut>>,
    write_de: Option<DeflateEncoder<BinaryMut>>,
}

impl Debug for InnerCompress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerCompress")
            .field("write_gz", &self.write_gz)
            .field("write_de", &self.write_de)
            .finish()
    }
}

impl InnerCompress {
    pub fn new() -> Self {
        Self {
            write_gz: None,
            write_br: None,
            write_de: None,
        }
    }

    pub fn open_write_gz(&mut self) {
        if self.write_gz.is_none() {
            self.write_gz = Some(GzEncoder::new(BinaryMut::new(), Compression::default()));
        }
    }

    pub fn open_write_de(&mut self) {
        if self.write_de.is_none() {
            self.write_de = Some(DeflateEncoder::new(
                BinaryMut::new(),
                Compression::default(),
            ));
        }
    }

    pub fn open_write_br(&mut self) {
        if self.write_br.is_none() {
            self.write_br = Some(CompressorWriter::new(BinaryMut::new(), 4096, 11, 22));
        }
    }
}


struct InnerDecompress {
    reader_gz: Option<GzDecoder<BinaryMut>>,
    reader_br: Option<Decompressor<BinaryMut>>,
    reader_de: Option<DeflateDecoder<BinaryMut>>,
}

impl Debug for InnerDecompress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerDecompress")
            .field("reader_gz", &self.reader_gz)
            .field("reader_de", &self.reader_de)
            .finish()
    }
}


impl InnerDecompress {
    pub fn new() -> Self {
        Self {
            reader_gz: None,
            reader_br: None,
            reader_de: None,
        }
    }

    pub fn open_reader_gz(&mut self, bin: BinaryMut) {
        if self.reader_gz.is_none() {
            self.reader_gz = Some(GzDecoder::new(bin));
        }
    }

    pub fn open_reader_de(&mut self, bin: BinaryMut) {
        if self.reader_de.is_none() {
            self.reader_de = Some(DeflateDecoder::new(
                bin,
            ));
        }
    }

    pub fn open_reader_br(&mut self, bin: BinaryMut) {
        if self.reader_br.is_none() {
            self.reader_br = Some(Decompressor::new(bin, 4096));
        }
    }
}

#[derive(Debug)]
pub struct RecvStream {
    receiver: InnerReceiver,
    read_buf: Option<BinaryMut>,
    cache_body_data: BinaryMut,
    origin_compress_method: i8,
    now_compress_method: i8,
    compress: InnerCompress,
    decompress: InnerDecompress,
    is_chunked: bool,
    is_end: bool,
    is_process_end: bool,
}

impl Default for RecvStream {
    fn default() -> Self {
        Self {
            receiver: InnerReceiver::new(),
            read_buf: Default::default(),
            cache_body_data: BinaryMut::new(),
            origin_compress_method: Consts::COMPRESS_METHOD_NONE,
            now_compress_method: Consts::COMPRESS_METHOD_NONE,
            compress: InnerCompress::new(),
            decompress: InnerDecompress::new(),
            is_chunked: false,
            is_end: true,
            is_process_end: false,
        }
    }
}

impl RecvStream {
    pub fn empty() -> RecvStream {
        Default::default()
    }

    pub fn only(binary: Binary) -> RecvStream {
        RecvStream {
            read_buf: Some(BinaryMut::from(binary)),
            ..Default::default()
        }
    }

    pub fn new(receiver: Receiver<(bool, Binary)>, binary: BinaryMut, is_end: bool) -> RecvStream {
        RecvStream {
            receiver: InnerReceiver::new_receiver(receiver),
            read_buf: Some(binary),
            is_end,
            ..Default::default()
        }
    }

    pub fn new_file(file: File, binary: BinaryMut, is_end: bool) -> RecvStream {
        RecvStream {
            receiver: InnerReceiver::new_file(file),
            read_buf: Some(binary),
            is_end,
            ..Default::default()
        }
    }

    pub fn binary(&mut self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = self.read_buf.take() {
            buffer.put_slice(bin.chunk());
        }
        buffer.freeze()
    }

    pub fn get_now_compress(&self) -> i8 {
        // 输入输出同一种编码, 不做任何处理
        if self.origin_compress_method == self.now_compress_method {
            return 0;
        }
        self.now_compress_method
    }

    pub fn set_compress_gzip(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_GZIP;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_deflate(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_DEFLATE;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_brotli(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_BROTLI;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_origin_gzip(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_GZIP;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_origin_deflate(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_DEFLATE;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_origin_brotli(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_BROTLI;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_origin_compress_method(&mut self, method: i8) {
        self.origin_compress_method = method;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn add_compress_method(&mut self, method: i8) -> i8 {
        self.now_compress_method = method;
        self.get_now_compress()
    }

    pub fn is_chunked(&mut self) -> bool {
        self.is_chunked
    }

    pub fn set_chunked(&mut self, chunked: bool) {
        self.is_chunked = chunked;
    }

    pub fn cache_buffer(&mut self, buf: &[u8]) -> usize {
        if self.read_buf.is_none() {
            self.read_buf = Some(BinaryMut::new());
        }
        self.read_buf.as_mut().unwrap().put_slice(buf);
        buf.len()
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn set_end(&mut self, end: bool) {
        self.is_end = end
    }

    pub fn read_now(&mut self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = self.read_buf.take() {
            buffer.put_slice(bin.chunk());
        }
        return buffer.freeze();
    }

    pub fn origin_len(&self) -> usize {
        let mut size = 0;
        if let Some(bin) = &self.read_buf {
            size += bin.remaining();
        }
        return size;
    }

    pub fn copy_now(&self) -> Binary {
        let mut buffer = BinaryMut::new();
        if let Some(bin) = &self.read_buf {
            buffer.put_slice(bin.chunk());
        }
        return buffer.freeze();
    }

    pub fn body_len(&mut self) -> usize {
        return self.cache_body_data.remaining();
    }

    pub async fn wait_all(&mut self) -> Option<usize> {
        let mut size = 0;
        if !self.is_end && !self.receiver.is_none() {
            while let Some(v) = self.receiver.recv().await {
                size += self.cache_buffer(v.1.chunk());
                self.is_end = v.0;
                if self.is_end == true {
                    break;
                }
            }
        }
        Some(size)
    }

    pub async fn read_all(&mut self, buffer: &mut BinaryMut) -> Option<usize> {
        if !self.is_end && !self.receiver.is_none() {
            while let Some(v) = self.receiver.recv().await {
                self.cache_buffer(v.1.chunk());
                self.is_end = v.0;
                if self.is_end == true {
                    break;
                }
            }
        }
        let _ = self.process_data(None);
        match self.read_data(buffer) {
            Ok(s) => Some(s),
            _ => None,
        }
    }

    fn inner_encode_write_data<B: webparse::Buf + webparse::BufMut>(
        buffer: &mut B,
        data: &[u8],
        is_chunked: bool,
    ) -> std::io::Result<usize> {
        if is_chunked {
            Helper::encode_chunk_data(buffer, data)
        } else {
            Ok(buffer.put_slice(data))
        }
    }

    fn encode_write_data(&mut self, data: &[u8]) -> std::io::Result<usize> {
        match self.get_now_compress() {
            Consts::COMPRESS_METHOD_GZIP => {
                // 数据结束，需要主动调用结束以导出全部结果
                if data.len() == 0 {
                    self.compress.open_write_gz();
                    let gz = self.compress.write_gz.take().unwrap();
                    let value = gz.finish().unwrap();
                    if value.remaining() > 0 {
                        Self::inner_encode_write_data(
                            &mut self.cache_body_data,
                            &value,
                            self.is_chunked,
                        )?;
                    }
                    if self.is_chunked {
                        Helper::encode_chunk_data(&mut self.cache_body_data, data)
                    } else {
                        Ok(0)
                    }
                } else {
                    self.compress.open_write_gz();
                    let gz = self.compress.write_gz.as_mut().unwrap();
                    gz.write_all(data).unwrap();
                    // 每次写入，在尝试读取出数据
                    if gz.get_mut().remaining() > 0 {
                        let s = Self::inner_encode_write_data(
                            &mut self.cache_body_data,
                            &gz.get_mut().chunk(),
                            self.is_chunked,
                        );
                        gz.get_mut().clear();
                        s
                    } else {
                        Ok(0)
                    }
                }
            }
            Consts::COMPRESS_METHOD_DEFLATE => {
                // 数据结束，需要主动调用结束以导出全部结果
                if data.len() == 0 {
                    self.compress.open_write_de();
                    let de = self.compress.write_de.take().unwrap();
                    let value = de.finish().unwrap();
                    if value.remaining() > 0 {
                        Self::inner_encode_write_data(
                            &mut self.cache_body_data,
                            &value,
                            self.is_chunked,
                        )?;
                    }
                    if self.is_chunked {
                        Helper::encode_chunk_data(&mut self.cache_body_data, data)
                    } else {
                        Ok(0)
                    }
                } else {
                    self.compress.open_write_de();
                    let de = self.compress.write_de.as_mut().unwrap();
                    de.write_all(data).unwrap();
                    // 每次写入，在尝试读取出数据
                    if de.get_mut().remaining() > 0 {
                        let s = Self::inner_encode_write_data(
                            &mut self.cache_body_data,
                            &de.get_mut().chunk(),
                            self.is_chunked,
                        );
                        de.get_mut().clear();
                        s
                    } else {
                        Ok(0)
                    }
                }
            }
            Consts::COMPRESS_METHOD_BROTLI => {
                // 数据结束，需要主动调用结束以导出全部结果
                if data.len() == 0 {
                    self.compress.open_write_br();
                    let mut de = self.compress.write_br.take().unwrap();
                    de.flush()?;
                    let value = de.into_inner();
                    if value.remaining() > 0 {
                        Self::inner_encode_write_data(
                            &mut self.cache_body_data,
                            &value,
                            self.is_chunked,
                        )?;
                    }
                    if self.is_chunked {
                        Helper::encode_chunk_data(&mut self.cache_body_data, data)
                    } else {
                        Ok(0)
                    }
                } else {
                    self.compress.open_write_br();
                    let de = self.compress.write_br.as_mut().unwrap();
                    de.write_all(data).unwrap();
                    // 每次写入，在尝试读取出数据
                    if de.get_mut().remaining() > 0 {
                        let s = Self::inner_encode_write_data(
                            &mut self.cache_body_data,
                            &de.get_mut().chunk(),
                            self.is_chunked,
                        );
                        de.get_mut().clear();
                        s
                    } else {
                        Ok(0)
                    }
                }
            }
            _ => Self::inner_encode_write_data(&mut self.cache_body_data, data, self.is_chunked),
        }
    }

    pub fn poll_encode_write<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &mut B,
    ) -> Poll<webparse::WebResult<usize>> {
        self.process_data(Some(cx))?;
        let s = self.read_data(buffer)?;
        Poll::Ready(Ok(s))
    }

    fn inner_poll_read(&mut self, cx: &mut Context<'_>) -> Poll<std::io::Result<bool>> {
        if self.is_end {
            return Poll::Ready(Ok(false));
        }
        let mut has_change = false;
        loop {
            match self.receiver.poll_recv(cx) {
                Poll::Ready(Some((is_end, bin))) => {
                    self.is_end = is_end;
                    self.cache_buffer(&bin.chunk());
                    has_change = true;
                    if self.is_end {
                        break;
                    }
                }
                Poll::Ready(None) => {
                    self.is_end = true;
                    has_change = true;
                    break;
                }
                Poll::Pending => break,
            }
        }
        return Poll::Ready(Ok(has_change));
    }

    /// 返回true表示需要等待, 否则继续执行
    fn inner_decode_data(&mut self) -> webparse::WebResult<bool> {
        // 无数据
        if self.read_buf.is_none() {
            return Ok(true)
        }
        // 原始的压缩方式不为空, 表示数据可能需要处理
        if self.origin_compress_method != Consts::COMPRESS_METHOD_NONE {
            // 数据方式与原有的一模一样, 不做处理
            if self.origin_compress_method == self.now_compress_method {
                return Ok(false)
            }
            // 数据结束前不做解压缩操作, 后续也不可读
            if self.is_end {
                return Ok(true)
            }
            let _size = match self.origin_compress_method {
                Consts::COMPRESS_METHOD_GZIP => {
                    self.decompress.open_reader_gz(self.read_buf.clone().unwrap());
                    let gz = self.decompress.reader_gz.as_mut().unwrap();
                    let mut read_buf = BinaryMut::new();
                    let s = read_all_data(&mut read_buf, gz)?;
                    self.read_buf = Some(read_buf);
                    s
                },
                Consts::COMPRESS_METHOD_DEFLATE => {
                    self.decompress.open_reader_de(self.read_buf.clone().unwrap());
                    let de = self.decompress.reader_de.as_mut().unwrap();
                    let mut read_buf = BinaryMut::new();
                    let s = read_all_data(&mut read_buf, de)?;
                    self.read_buf = Some(read_buf);
                    s
                },
                Consts::COMPRESS_METHOD_BROTLI => {
                    self.decompress.open_reader_br(self.read_buf.clone().unwrap());
                    let br = self.decompress.reader_br.as_mut().unwrap();
                    let mut read_buf = BinaryMut::new();
                    let s = read_all_data(&mut read_buf, br)?;
                    self.read_buf = Some(read_buf);
                    s
                },
                _ => {
                    return Err(WebError::Extension("未知的压缩格式"));
                }
            };
            self.origin_compress_method = Consts::COMPRESS_METHOD_NONE;
        }

        Ok(false)
    }

    pub fn process_data(&mut self, mut cx: Option<&mut Context<'_>>) -> webparse::WebResult<usize> {
        if self.is_process_end {
            return Ok(0);
        }
        if self.inner_decode_data()? {
            return Ok(0);
        }

        let mut size = 0;
        if let Some(bin) = self.read_buf.take() {
            if bin.chunk().len() > 0 {
                size += self.encode_write_data(bin.chunk())?;
            }
        }
        let mut has_encode_end = false;
        if !self.is_end && cx.is_some() {
            loop {
                match self.receiver.poll_recv(cx.as_mut().unwrap()) {
                    Poll::Pending => {
                        println!("fuck!!!!!!!!!!!!!!!");
                        break;
                    }
                    Poll::Ready(Some((is_end, bin))) => {
                        size += self.encode_write_data(bin.chunk())?;
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
        if !has_encode_end && self.is_end {
            self.encode_write_data(&[])?;
        }
        self.is_process_end = has_encode_end || self.is_end;
        Ok(size)
    }

    pub fn read_data<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        read_data: &mut B,
    ) -> WebResult<usize> {
        self.process_data(None)?;

        let mut size = 0;
        if self.cache_body_data.remaining() > 0 {
            size += read_data.put_slice(&self.cache_body_data.chunk());
            self.cache_body_data.advance_all();
        }
        Ok(size)
    }
}

impl AsyncRead for RecvStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match ready!(self.inner_poll_read(cx)?) {
            true => {}
            false => return Poll::Pending,
        };
        self.process_data(Some(cx)).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "process data error"))?;
        let len = std::cmp::min(self.cache_body_data.remaining(), buf.remaining());
        buf.put_slice(&self.cache_body_data.chunk()[..len]);
        self.cache_body_data.advance(len);
        return Poll::Ready(Ok(()));
    }
}

impl Serialize for RecvStream {
    fn serialize<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        buffer: &mut B,
    ) -> webparse::WebResult<usize> {
        let mut size = 0;
        if let Some(bin) = self.read_buf.take() {
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
                f.field("接收字节数", &self.cache_body_data.remaining());
            }
            f.finish()
        }
    }
}
