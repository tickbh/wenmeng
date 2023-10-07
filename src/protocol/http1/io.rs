use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};



use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::mpsc::Sender,
};

use webparse::{
    http::http2, Binary, BinaryMut, Buf, BufMut, Request, Response, Helper, WebError, HttpError,
};
use crate::{ProtError, ProtResult, RecvStream};

pub struct IoBuffer<T> {
    io: T,
    read_buf: BinaryMut,
    write_buf: BinaryMut,

    inner: ConnectionInfo,
}

struct ConnectionInfo {
    deal_req: usize,
    read_sender: Option<Sender<(bool, Binary)>>,
    res: Option<Response<RecvStream>>,
    req: Option<Request<RecvStream>>,
    is_keep_alive: bool,
    is_send_body: bool,
    is_send_header: bool,
    is_build_req: bool,
    is_send_end: bool,
    left_body_len: usize,
    is_delay_close: bool,
    is_now_chunked: bool,
}

impl ConnectionInfo {

    pub fn is_active_close(&self) -> bool {
        self.is_send_end && !self.is_keep_alive
    }
}

impl<T> IoBuffer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        Self {
            io: io,
            read_buf: BinaryMut::new(),
            write_buf: BinaryMut::new(),

            inner: ConnectionInfo {
                deal_req: 0,
                read_sender: None,
                res: None,
                req: None,
                is_keep_alive: false,
                is_send_body: false,
                is_send_header: false,
                is_build_req: false,
                is_send_end: false,
                is_delay_close: false,
                left_body_len: usize::MAX,
                is_now_chunked: false,
            },
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        if let Some(res) = &mut self.inner.res {
            if !self.inner.is_send_header {
                res.encode_header(&mut self.write_buf)?;
                self.inner.is_send_header = true;
            }

            if !res.body().is_end() || !self.inner.is_send_body {
                self.inner.is_send_body = true;
                let _ = res.body_mut().poll_encode(cx, &mut self.write_buf);
                if res.body().is_end() {
                    self.inner.is_send_end = true;
                    self.inner.deal_req += 1;
                }
            }
        }

        if let Some(req) = &mut self.inner.req {
            if !self.inner.is_send_header {
                req.encode_header(&mut self.write_buf)?;
                self.inner.is_send_header = true;
            }

            if !req.body().is_end() || !self.inner.is_send_body {
                self.inner.is_send_body = true;
                let _ = req.body_mut().poll_encode(cx, &mut self.write_buf);
                if req.body().is_end() {
                    self.inner.is_send_end = true;
                    self.inner.deal_req += 1;
                }
            }
        }

        if self.inner.is_send_end {
            self.inner.res = None;
            self.inner.req = None;
            self.inner.is_build_req = false;
            self.inner.is_now_chunked = false;
            self.inner.is_send_header = false;
        }

        if self.write_buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        match ready!(Pin::new(&mut self.io).poll_write(cx, &self.write_buf.chunk()))? {
            n => {
                self.write_buf.advance(n);
                if self.write_buf.is_empty() {
                    return Poll::Ready(Ok(n));
                }
            }
        };
        Poll::Pending
    }

    pub fn poll_read(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        self.read_buf.reserve(1);
        let n = {
            let mut buf = ReadBuf::uninit(self.read_buf.chunk_mut());
            let ptr = buf.filled().as_ptr();
            ready!(Pin::new(&mut self.io).poll_read(cx, &mut buf)?);
            assert_eq!(ptr, buf.filled().as_ptr());
            buf.filled().len()
        };

        unsafe {
            self.read_buf.advance_mut(n);
        }
        Poll::Ready(Ok(n))
    }

    pub fn poll_read_all(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        let mut size = 0;
        loop {
            match self.poll_read(cx)? {
                Poll::Ready(0) => return Poll::Ready(Ok(0)),
                Poll::Ready(n) => size += n,
                Poll::Pending => {
                    if size == 0 {
                        return Poll::Pending;
                    } else {
                        break;
                    }
                }
            }
        }
        Poll::Ready(Ok(size))
    }

    pub fn receive_body_len(&mut self, body_len: usize) -> bool {
        println!("left len = {}, reciver = {}", self.inner.left_body_len, body_len);
        if self.inner.left_body_len <= body_len {
            self.inner.left_body_len = 0;
            true
        } else {
            self.inner.left_body_len -= body_len;
            false
        }
    }

    pub fn poll_request(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Request<RecvStream>>>> {
        let n = self.poll_write(cx)?;
        if n == Poll::Ready(0) && self.inner.is_active_close() && self.write_buf.is_empty() {
            return Poll::Ready(None);
        }
        match ready!(self.poll_read_all(cx)?) {
            // socket被断开, 提前结束
            0 => {
                log::trace!("read socket zero, now close socket");
                return Poll::Ready(None);
            }
            // 收到新的消息头, 解析包体消息
            _ => {
                if self.inner.is_build_req {
                    self.do_deal_body()?;
                    return Poll::Pending;
                }
                let mut request = Request::new();
                let size = match request.parse_buffer(&mut self.read_buf.clone()) {
                    Err(e) => {
                        if e.is_partial() {
                            return Poll::Pending;
                        } else {
                            if self.read_buf.remaining() >= http2::MAIGC_LEN
                                && &self.read_buf[..http2::MAIGC_LEN] == http2::HTTP2_MAGIC
                            {
                                self.read_buf.advance(http2::MAIGC_LEN);
                                let err = ProtError::ServerUpgradeHttp2(Binary::new(), None);
                                return Poll::Ready(Some(Err(err)));
                            }
                            return Poll::Ready(Some(Err(e.into())));
                        }
                    }
                    Ok(n) => n,
                };
                // let size = request.parse_buffer(&mut self.read_buf.clone())?;
                if request.is_partial() {
                    return Poll::Pending;
                }

                self.read_buf.advance(size);
                self.inner.is_send_body = false;
                self.inner.is_send_end = false;
                self.inner.is_build_req = true;
                self.inner.is_keep_alive = request.is_keep_alive();
                let body_len = request.get_body_len();
                self.inner.left_body_len = if body_len < 0 { usize::MAX } else { body_len as usize };
                let new_request = if body_len == 0 {
                    request.into(RecvStream::empty()).0
                } else {
                    let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
                    self.inner.read_sender = Some(sender);
                    let mut binary = BinaryMut::new();
                    binary.put_slice(self.read_buf.chunk());
                    self.read_buf.advance(self.read_buf.remaining());
                    let is_end = self.receive_body_len(binary.remaining());
                    request.into(RecvStream::new(receiver, binary, is_end)).0
                };
                return Poll::Ready(Some(Ok(new_request)));
            }
        }
    }



    pub fn do_deal_body(&mut self) -> ProtResult<bool> {
            // chunk 格式数据
        let mut all_is_end = false;
        if self.inner.is_now_chunked {
            if let Some(sender) = &self.inner.read_sender {
                if let Ok(p) = sender.try_reserve() {
                    match Helper::parse_chunk_data(&mut self.read_buf.clone()) {
                        Ok((data, n, is_end)) => {
                            self.read_buf.advance(n);
                            p.send((is_end, Binary::from(data)));
                            all_is_end = is_end
                        },
                        Err(WebError::Http(HttpError::Partial)) => return Ok(false),
                        Err(err) => return Err(err.into()),
                    }
                }
            }
        } else {
            if let Some(sender) = self.inner.read_sender.take() {
                if let Ok(p) = sender.try_reserve() {
                    let binary = Binary::from(self.read_buf.chunk().to_vec());
                    let is_end = self.receive_body_len(binary.len());
                    p.send((is_end, binary));
                    self.read_buf.advance_all();
                    self.read_buf.clear();
                    all_is_end = is_end
                }
                self.inner.read_sender = Some(sender);
            }
        };


        if all_is_end && self.inner.is_active_close() && self.write_buf.is_empty() {
            println!("ddddd");
            return Ok(true);
        }
        println!("aaa");
        if self.inner.is_delay_close {
            return Ok(true);
        } else {
            return Ok(false);
        }
    }


    pub fn poll_response(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<ProtResult<Response<RecvStream>>>> {
        let n = self.poll_write(cx)?;
        // if n == Poll::Ready(0) && self.inner.is_active_close() && self.write_buf.is_empty() {
        //     println!("ddddd");
        //     return Poll::Ready(None);
        // }
        if self.inner.is_delay_close {
            return Poll::Ready(None);
        }
        match ready!(self.poll_read_all(cx)?) {
            // socket被断开, 提前结束
            // 0 => {
            //     log::trace!("read socket zero, now close socket");
            //     println!("bbbb");
            //     self.inner.is_delay_close = true;
            //     return Poll::Ready(None);
            // }
            // 收到新的消息头, 解析包体消息
            n @ _ => {
                if n == 0 {
                    self.inner.is_delay_close = true;
                }
                if self.inner.is_build_req {
                    let is_close = self.do_deal_body()?;
                    if is_close {
                        return Poll::Ready(None);
                    } else {
                        return Poll::Pending;
                    }
                }
                let mut response = Response::new(());
                let size = match response.parse_buffer(&mut self.read_buf.clone()) {
                    Err(e) => {
                        if e.is_partial() {
                            if self.inner.is_delay_close {
                                return Poll::Ready(None);
                            } else {
                                return Poll::Pending;
                            }
                        } else {
                            return Poll::Ready(Some(Err(e.into())));
                        }
                    }
                    Ok(n) => n,
                };

                if response.is_partial() {
                    if self.inner.is_delay_close {
                        return Poll::Ready(None);
                    } else {
                        return Poll::Pending;
                    }
                }

                self.read_buf.advance(size);
                self.inner.is_send_body = false;
                self.inner.is_send_end = false;
                self.inner.is_build_req = true;
                // self.inner.is_keep_alive = response.is_keep_alive();
                let body_len = response.get_body_len();
                self.inner.left_body_len = if body_len < 0 { usize::MAX } else { body_len as usize };
                if response.status().is_success() && body_len == 0 {
                    if response.headers().is_chunk() {
                        self.inner.is_now_chunked = true;
                    } else {
                        self.inner.left_body_len = usize::MAX;
                    }
                }
                println!("body len = {:?}", self.inner.left_body_len);
                if self.inner.left_body_len == 0 {
                    let new_response = response.into(RecvStream::empty()).0;
                    return Poll::Ready(Some(Ok(new_response)));
                } else {
                    let new_response = {
                        let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
                        self.inner.read_sender = Some(sender);
                        let mut binary = BinaryMut::new();
                        let is_end = if self.inner.left_body_len > self.read_buf.remaining() {
                            binary.put_slice(self.read_buf.chunk());
                            self.read_buf.advance_all();
                            self.read_buf.clear();
                            self.receive_body_len(binary.remaining())
                        } else {
                            binary.put_slice(&self.read_buf.chunk()[0..self.inner.left_body_len]);
                            self.read_buf.advance(self.inner.left_body_len);
                            self.receive_body_len(self.inner.left_body_len)
                        };
                        response.into(RecvStream::new(receiver, binary, is_end)).0
                    };
                    return Poll::Ready(Some(Ok(new_response)));
                }
            }
        }
    }

    pub fn into(self) -> (T, BinaryMut, BinaryMut) {
        (self.io, self.read_buf, self.write_buf)
    }

    pub async fn send_response(&mut self, res: Response<RecvStream>) -> ProtResult<()> {
        self.inner.res = Some(res);
        Ok(())
    }

    pub async fn send_request(&mut self, req: Request<RecvStream>) -> ProtResult<()> {
        self.inner.req = Some(req);
        Ok(())
    }
}
