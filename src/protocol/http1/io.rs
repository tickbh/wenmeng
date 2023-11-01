use std::{
    pin::Pin,
    task::{ready, Context, Poll}, collections::LinkedList,
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::mpsc::Sender,
};

use crate::{ProtError, ProtResult, RecvStream, HeaderHelper, SendStream};
use webparse::{
    http::http2, Binary, BinaryMut, Buf, BufMut, Request, Response, Version,
};

pub struct IoBuffer<T> {
    io: T,
    send_stream: SendStream,
    write_buf: BinaryMut,

    inner: ConnectionInfo,
}

struct ConnectionInfo {
    deal_req: usize,
    read_sender: Option<Sender<(bool, Binary)>>,
    res: LinkedList<Response<RecvStream>>,
    req: LinkedList<Request<RecvStream>>,
    is_keep_alive: bool,
    is_delay_close: bool,

    req_status: SendStatus,
    res_status: SendStatus,
}

#[derive(Debug)]
struct SendStatus {
    pub is_send_body: bool,
    pub is_send_header: bool,
    pub is_send_finish: bool,

    pub is_read_header_end: bool,
    pub is_read_finish: bool,
    pub left_read_body_len: usize,
    pub is_chunked: bool,
}

impl Default for SendStatus {
    fn default() -> Self {
        Self {
            is_send_body: Default::default(),
            is_send_header: Default::default(),
            is_send_finish: Default::default(),

            is_read_header_end: Default::default(),
            is_read_finish: Default::default(),
            left_read_body_len: Default::default(),
            is_chunked: Default::default(),
        }
    }
}

impl SendStatus {
    pub fn clear(&mut self) {
        self.clear_read();
        self.clear_write();
    }

    pub fn clear_write(&mut self) {
        self.is_send_body = false;
        self.is_send_header = false;
        self.is_send_finish = false;
    }
    
    pub fn clear_read(&mut self) {
        self.is_read_finish = false;
        self.is_read_header_end = false;
        self.left_read_body_len = 0;
        self.is_chunked = false;
    }
}

impl ConnectionInfo {
    pub fn is_active_close(&self) -> bool {
        self.req_status.is_send_finish && self.req_status.is_send_finish && !self.is_keep_alive
    }
}

impl<T> IoBuffer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(io: T) -> Self {
        Self {
            io: io,
            send_stream: SendStream::empty(),
            write_buf: BinaryMut::new(),

            inner: ConnectionInfo {
                deal_req: 0,
                read_sender: None,
                res: LinkedList::new(),
                req: LinkedList::new(),
                is_keep_alive: false,
                is_delay_close: false,

                req_status: SendStatus::default(),
                res_status: SendStatus::default(),
            },
        }
    }

    pub fn check_finish_status(&mut self) {
        if self.inner.req_status.is_send_finish && self.inner.res_status.is_send_finish {
            self.set_now_end();
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        if let Some(res) = self.inner.res.front_mut() {
            if !self.inner.res_status.is_send_header {
                self.inner.res_status.is_chunked = res.headers().is_chunked();
                HeaderHelper::process_response_header(Version::Http11, true, res)?;
                res.encode_header(&mut self.write_buf)?;
                self.inner.res_status.is_send_header = true;
            }

            if !res.body().is_end() || !self.inner.res_status.is_send_body {
                self.inner.res_status.is_send_body = true;
                let _ = res.body_mut().poll_encode_write(
                    cx,
                    &mut self.write_buf,
                );
            }

            if res.body().is_end() {
                self.inner.res_status.is_send_finish = true;
                self.inner.deal_req += 1;
            }
        }
        if self.inner.res_status.is_send_finish {
            self.inner.res.pop_front();
            self.inner.res_status.clear_write();
        }

        if let Some(req) = self.inner.req.front_mut() {
            if !self.inner.req_status.is_send_header {
                req.encode_header(&mut self.write_buf)?;
                self.inner.req_status.is_send_header = true;
            }

            if !req.body().is_end() || !self.inner.req_status.is_send_body {
                self.inner.req_status.is_send_body = true;
                let _ = req.body_mut().poll_encode_write(
                    cx,
                    &mut self.write_buf,
                );
            }
            if req.body().is_end() {
                self.inner.req_status.is_send_finish = true;
                self.inner.deal_req += 1;
            }
        }
        if self.inner.req_status.is_send_finish {
            self.inner.req.pop_front();
            self.inner.req_status.clear_write();
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
        self.send_stream.read_buf.reserve(1);
        let n = {
            let mut buf = ReadBuf::uninit(self.send_stream.read_buf.chunk_mut());
            let ptr = buf.filled().as_ptr();
            ready!(Pin::new(&mut self.io).poll_read(cx, &mut buf)?);
            assert_eq!(ptr, buf.filled().as_ptr());
            buf.filled().len()
        };

        unsafe {
            self.send_stream.read_buf.advance_mut(n);
        }
        self.send_stream.process_data()?;
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

    fn receive_body_len(status: &mut SendStatus, body_len: usize) -> bool {
        if status.left_read_body_len <= body_len {
            status.left_read_body_len = 0;
            true
        } else {
            status.left_read_body_len -= body_len;
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
            _n @ _ => {
                if self.inner.req_status.is_read_header_end {
                    self.do_deal_body(true)?;

                    if self.inner.req_status.is_read_finish {
                        self.inner.req_status.clear_read();
                        self.send_stream.set_end_headers(false);
                    }
                    // 如果还有数据可能是keep-alive继续读取头信息
                    if self.send_stream.read_buf.is_empty() && !self.inner.req_status.is_read_header_end {
                        return Poll::Pending;
                    }
                }
                let mut request = Request::new();
                let size = match request.parse_buffer(&mut self.send_stream.read_buf.clone()) {
                    Err(e) => {
                        if e.is_partial() {
                            return Poll::Pending;
                        } else {
                            if self.send_stream.read_buf.remaining() >= http2::MAIGC_LEN
                                && &self.send_stream.read_buf[..http2::MAIGC_LEN] == http2::HTTP2_MAGIC
                            {
                                // self.read_buf.advance(http2::MAIGC_LEN);
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
                self.send_stream.set_new_body();
                let method = HeaderHelper::get_compress_method(request.headers());

                self.send_stream.read_buf.advance(size);
                self.inner.req_status.is_send_body = false;
                self.inner.req_status.is_send_finish = false;
                self.inner.req_status.is_read_header_end = true;
                self.inner.is_keep_alive = request.is_keep_alive();
                let body_len = request.get_body_len();
                self.inner.req_status.left_read_body_len = if body_len < 0 {
                    usize::MAX
                } else {
                    body_len as usize
                };
                if !request.method().is_nobody() && body_len == 0 {
                    self.inner.req_status.left_read_body_len = usize::MAX;
                    if request.headers().is_chunked() {
                        self.inner.req_status.is_chunked = true;
                    }
                }

                let (mut recv, sender) =
                    Self::build_recv_stream(&mut self.inner.res_status, &mut self.send_stream)?;
                recv.set_origin_compress_method(method);
                if recv.is_end() {
                    self.inner.req_status.clear_read();
                    self.send_stream.set_end_headers(false);
                }
                self.inner.read_sender = sender;
                return Poll::Ready(Some(Ok(request.into(recv).0)));
            }
        }
    }

    pub fn do_deal_body(&mut self, is_req: bool) -> ProtResult<bool> {
        // chunk 格式数据
        let status = if is_req {
            &mut self.inner.req_status
        } else {
            &mut self.inner.res_status
        };
        if let Some(sender) = &self.inner.read_sender {
            loop {
                match sender.try_reserve() {
                    Ok(p) => {
                        let mut read_data = BinaryMut::new();
                        match self.send_stream.read_data(&mut read_data)? {
                            0 => return Ok(false),
                            _ => {
                                p.send((self.send_stream.is_end(), read_data.freeze()));
                                status.is_read_finish = self.send_stream.is_end();
                            }
                        }
                    }
                    Err(_) => return Err(ProtError::Extension("sender error")),
                }
            }
        }
        if self.inner.is_active_close() && self.write_buf.is_empty() {
            return Ok(true);
        }
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
        let _n = self.poll_write(cx)?;
        if self.inner.is_delay_close {
            return Poll::Ready(None);
        }
        match ready!(self.poll_read_all(cx)?) {
            // 收到新的消息头, 解析包体消息
            n @ _ => {
                if n == 0 {
                    self.inner.is_delay_close = true;
                }
                if self.inner.res_status.is_read_header_end {
                    let is_close = self.do_deal_body(false)?;
                    
                    if self.inner.res_status.is_read_finish {
                        self.inner.res_status.clear_read();
                    }
                    if is_close {
                        return Poll::Ready(None);
                    } else {
                        return Poll::Pending;
                    }
                }
                let mut response = Response::new(());
                let size = match response.parse_buffer(&mut self.send_stream.read_buf.clone()) {
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

                self.send_stream.set_new_body();
                let method = HeaderHelper::get_compress_method(response.headers());

                self.send_stream.read_buf.advance(size);
                self.inner.res_status.is_send_body = false;
                self.inner.res_status.is_send_finish = false;
                self.inner.res_status.is_read_header_end = true;
                // self.inner.res_status.is_keep_alive = response.is_keep_alive();
                let body_len = response.get_body_len();
                self.inner.res_status.left_read_body_len = if body_len < 0 {
                    usize::MAX
                } else {
                    body_len as usize
                };
                if response.status().is_success() && body_len == 0 {
                    self.inner.res_status.left_read_body_len = usize::MAX;
                    if response.headers().is_chunked() {
                        self.inner.res_status.is_chunked = true;
                    }
                }
                let (mut recv, sender) =
                    Self::build_recv_stream(&mut self.inner.res_status, &mut self.send_stream)?;
                recv.set_origin_compress_method(method);
                if recv.is_end() {
                    self.inner.res_status.clear_read();
                }
                self.inner.read_sender = sender;
                return Poll::Ready(Some(Ok(response.into(recv).0)));
            }
        }
    }

    fn build_recv_stream(
        status: &mut SendStatus,
        send_stream: &mut SendStream,
    ) -> ProtResult<(RecvStream, Option<Sender<(bool, Binary)>>)> {
        send_stream.set_left_body(status.left_read_body_len);
        send_stream.set_chunked(status.is_chunked);

        if status.left_read_body_len == 0 {
            return Ok((RecvStream::empty(), None));
        } else {
            send_stream.process_data()?;
            let mut read_data = BinaryMut::new();
            send_stream.read_data(&mut read_data)?;
            let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
            return Ok((RecvStream::new(receiver, read_data, send_stream.is_end()), Some(sender)));
        }
    }

    fn set_now_end(&mut self) {
        self.inner.req_status.clear();
        self.inner.res_status.clear();
    }

    pub fn into(self) -> (T, BinaryMut, BinaryMut) {
        (self.io, self.send_stream.read_buf, self.write_buf)
    }

    pub async fn send_response(&mut self, res: Response<RecvStream>) -> ProtResult<()> {
        self.check_finish_status();
        self.inner.res.push_back(res);
        self.inner.res_status.is_send_finish = false;
        Ok(())
    }

    pub fn send_request(&mut self, req: Request<RecvStream>) -> ProtResult<()> {
        self.check_finish_status();
        self.inner.req.push_back(req);
        self.inner.req_status.is_send_finish = false;
        Ok(())
    }
}
