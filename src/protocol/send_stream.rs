// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
// 
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
// 
// Author: tickbh
// -----
// Created Date: 2023/09/14 09:42:25

use futures_core::Stream;
use std::{fmt::Debug};
use std::io::Read;
use tokio::sync::mpsc::{Sender};
use webparse::{Binary, BinaryMut, Serialize, Buf, Helper, HttpError, WebError};

use crate::{ProtResult};

#[derive(Debug)]
pub struct SendStream {
    sender: Option<Sender<(bool, Binary)>>,
    pub read_buf: BinaryMut,
    real_read_buf: BinaryMut,
    is_chunked: bool,
    is_end: bool,
    is_end_headers: bool,
    left_read_body_len: usize,
}

impl SendStream {
    pub fn empty() -> SendStream {
        SendStream {
            sender: None,
            read_buf: BinaryMut::new(),
            real_read_buf: BinaryMut::new(),
            is_end: true,
            is_end_headers: false,
            is_chunked: false,
            left_read_body_len: 0,
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
                    Ok((use_size, chunk_size)) => {
                        self.is_end = chunk_size == 0;
                        self.read_buf.advance(use_size);
                        self.real_read_buf.put_slice(&self.read_buf.chunk()[..chunk_size]);
                        self.read_buf.advance(chunk_size);
                        Helper::skip_new_line(&mut self.read_buf)?;
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
                self.real_read_buf.put_slice(&self.read_buf.chunk()[..len]);
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
