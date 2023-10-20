use brotli::{Decompressor};
use flate2::{read::{MultiGzDecoder, DeflateDecoder}};
use futures_core::Stream;
use std::{fmt::Debug};
use std::io::Read;
use tokio::sync::mpsc::{Sender};
use webparse::{Binary, BinaryMut, Serialize, Buf, Helper, HttpError, WebError};

use crate::{ProtResult, Consts};

struct InnerCompress {
    reader_gz: Option<MultiGzDecoder<BinaryMut>>,
    reader_br: Option<Decompressor<BinaryMut>>,
    reader_de: Option<DeflateDecoder<BinaryMut>>,
}

impl Debug for InnerCompress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerCompress")
            .field("reader_gz", &self.reader_gz)
            .field("reader_de", &self.reader_de)
            .finish()
    }
}


impl InnerCompress {
    pub fn new() -> Self {
        Self {
            reader_gz: None,
            reader_br: None,
            reader_de: None,
        }
    }

    pub fn open_reader_gz(&mut self, bin: BinaryMut) {
        if self.reader_gz.is_none() {
            self.reader_gz = Some(MultiGzDecoder::new(bin));
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
pub struct SendStream {
    sender: Option<Sender<(bool, Binary)>>,
    compress: InnerCompress,
    pub read_buf: BinaryMut,
    real_read_buf: BinaryMut,
    cache_body_data: BinaryMut,
    origin_compress_method: i8,
    now_compress_method: i8,
    is_chunked: bool,
    is_end: bool,
    is_end_headers: bool,
    left_read_body_len: usize,
    left_chunk_len: usize,
    cache_buf: Vec<u8>,
}

impl SendStream {
    pub fn empty() -> SendStream {
        SendStream {
            sender: None,
            compress: InnerCompress::new(),
            read_buf: BinaryMut::new(),
            real_read_buf: BinaryMut::new(),
            cache_body_data: BinaryMut::new(),
            origin_compress_method: Consts::COMPRESS_METHOD_NONE,
            now_compress_method: Consts::COMPRESS_METHOD_NONE,
            is_end: true,
            is_end_headers: false,
            is_chunked: false,
            left_read_body_len: 0,
            left_chunk_len: 0,
            cache_buf: Vec::new(),
        }
    }

    pub fn new(sender: Sender<(bool, Binary)>) -> SendStream {
        SendStream {
            sender: Some(sender),
            ..Self::empty()
        }
    }

    pub fn set_new_body(&mut self) {
        self.is_end_headers = true;
        self.is_end = false;
        self.is_chunked = false;
        self.left_read_body_len = 0;
        self.origin_compress_method = Consts::COMPRESS_METHOD_NONE;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_left_body(&mut self, left_read_body_len: usize) {
        self.left_read_body_len = left_read_body_len;
        self.is_end = false;
    }

    pub fn set_chunked(&mut self, chunked: bool) {
        self.is_chunked = chunked;
    }

    
    pub fn set_end_headers(&mut self, is_end_headers: bool) {
        self.is_end_headers = is_end_headers;
    }

    
    pub fn get_now_compress(&self) -> i8 {
        if self.origin_compress_method > 0 {
            return self.origin_compress_method
        } else {
            self.origin_compress_method + self.now_compress_method
        }
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
        self.origin_compress_method = Consts::COMPRESS_METHOD_ORIGIN_GZIP;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_origin_deflate(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_ORIGIN_DEFLATE;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_origin_brotli(&mut self) {
        self.origin_compress_method = Consts::COMPRESS_METHOD_ORIGIN_BROTLI;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn set_compress_method(&mut self, method: i8) {
        self.origin_compress_method = method;
        self.now_compress_method = Consts::COMPRESS_METHOD_NONE;
    }

    pub fn add_compress_method(&mut self, method: i8) -> i8 {
        self.now_compress_method = method;
        self.get_now_compress()
    }

    fn read_all_data<R: Read>(cache_buf: &mut Vec<u8>, read_buf: &mut BinaryMut, mut read: R) -> webparse::WebResult<usize> {
        if cache_buf.len() != 4096 {
            cache_buf.resize(4096, 0);
        }
        let mut size = 0;
        loop {
            let s = read.read(cache_buf)?;
            size += s;
            read_buf.put_slice(&cache_buf[..s]);
            if s < cache_buf.len() {
                return Ok(size)
            }
        }
    }
    
    fn decode_data(
        &mut self,
        data: &[u8],
    ) -> webparse::WebResult<usize> {
        if self.get_now_compress() != 0 && self.cache_buf.len() != 4096 {
            self.cache_buf.resize(4096, 0);
        }
        // 收到数据进行缓存，只有到结束时才进行解压缩
        match self.get_now_compress() {
            Consts::COMPRESS_METHOD_GZIP => {
                self.cache_body_data.put_slice(data);
                if self.is_end {
                    self.compress.open_reader_gz(self.cache_body_data.clone());
                    let gz = self.compress.reader_gz.as_mut().unwrap();
                    let s = Self::read_all_data(&mut self.cache_buf, &mut self.real_read_buf, gz);
                    self.cache_body_data.clear();
                    s
                } else {
                    Ok(0)
                }
            }
            Consts::COMPRESS_METHOD_DEFLATE => {
                self.cache_body_data.put_slice(data);
                if self.is_end {
                    self.compress.open_reader_de(self.cache_body_data.clone());
                    let de = self.compress.reader_de.as_mut().unwrap();
                    let s = Self::read_all_data(&mut self.cache_buf, &mut self.real_read_buf, de);
                    self.cache_body_data.clear();
                    s
                } else {
                    Ok(0)
                }
            }
            Consts::COMPRESS_METHOD_BROTLI => {
                self.cache_body_data.put_slice(data);
                if self.is_end {
                    self.compress.open_reader_br(self.cache_body_data.clone());
                    let br = self.compress.reader_br.as_mut().unwrap();
                    let s = Self::read_all_data(&mut self.cache_buf, &mut self.real_read_buf, br);
                    self.cache_body_data.clear();
                    s
                } else {
                    Ok(0)
                }
            }
            _ => {
                self.real_read_buf.put_slice(data);
                Ok(data.len())
            },
        }
    }

    pub fn process_data(&mut self) -> ProtResult<()> {
        // 头部数据不做处理
        if !self.is_end_headers {
            return Ok(())
        }
        loop {
            if self.is_chunked {
                if self.is_end {
                    return Ok(())
                }
                // TODO 接收小部分的chunk
                match Helper::parse_chunk_data(&mut self.read_buf.clone()) {
                    Ok((data, n, is_end)) => {
                        self.is_end = is_end;
                        self.decode_data(&data)?;
                        self.read_buf.advance(n);
                    }
                    Err(WebError::Http(HttpError::Partial)) => break,
                    Err(err) => return Err(err.into()),
                }
            } else {
                let len = std::cmp::min(self.left_read_body_len, self.read_buf.remaining());
                if len == 0 {
                    return Ok(())
                }
                self.left_read_body_len -= len;
                if self.left_read_body_len == 0 {
                    self.is_end = true;
                }
                self.decode_data(&self.read_buf.clone().chunk()[..len])?;
                self.read_buf.advance(len);
                break;
            }
        }
        Ok(())
    }

    pub fn read_data<B: webparse::Buf + webparse::BufMut>(&mut self, read_data: &mut B) -> ProtResult<usize> {
        self.process_data()?;

        let mut size = 0;
        if self.real_read_buf.remaining() > 0 {
            size += read_data.put_slice(&self.real_read_buf.chunk());
            self.real_read_buf.advance_all();
        }
        Ok(size)
    }

    // /// 返回Some则表示数据发送不成功，需要重新进行投递
    // pub fn send_data(&mut self, binary: Binary, is_end_stream: bool) -> Option<(bool, Binary)> {
    //     if let Some(Err(e)) = self
    //         .sender
    //         .as_ref()
    //         .map(|s| s.try_send((is_end_stream, binary)))
    //     {
    //         return Some(match e {
    //             TrySendError::Closed(v) => v,
    //             TrySendError::Full(v) => v,
    //         });
    //     }
    //     None
    // }

    pub fn is_end(&self) -> bool {
        self.is_end
    }
}

impl Stream for SendStream {
    type Item = ProtResult<Binary>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

impl Read for SendStream {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

impl Serialize for SendStream {
    fn serialize<B: webparse::Buf + webparse::BufMut>(
        &mut self,
        _buffer: &mut B,
    ) -> webparse::WebResult<usize> {
        Ok(0)
    }
}

unsafe impl Sync for SendStream {}

unsafe impl Send for SendStream {}
