use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::mpsc::Sender,
};

use crate::{ProtError, ProtResult, RecvStream};
use webparse::{
    http::http2, Binary, BinaryMut, Buf, BufMut, Helper, HttpError, Request, Response, WebError,
};

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
    is_delay_close: bool,

    req_status: SendStatus,
    res_status: SendStatus,
}

#[derive(Debug)]
struct SendStatus {
    pub is_send_body: bool,
    pub is_send_header: bool,
    pub is_build_header: bool,
    pub is_send_end: bool,
    pub left_body_len: usize,
    pub is_chunked: bool,
}

impl Default for SendStatus {
    fn default() -> Self {
        Self {
            is_send_body: Default::default(),
            is_send_header: Default::default(),
            is_build_header: Default::default(),
            is_send_end: false,
            left_body_len: Default::default(),
            is_chunked: Default::default(),
        }
    }
}

impl SendStatus {
    pub fn clear(&mut self) {
        self.is_send_body = false;
        self.is_send_header = false;
        self.is_build_header = false;
        self.is_send_end = false;
        self.left_body_len = 0;
        self.is_chunked = false;
    }
}

impl ConnectionInfo {
    pub fn is_active_close(&self) -> bool {
        self.req_status.is_send_end && self.req_status.is_send_end && !self.is_keep_alive
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
                is_delay_close: false,

                req_status: SendStatus::default(),
                res_status: SendStatus::default(),
            },
        }
    }

    pub fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<ProtResult<usize>> {
        if let Some(res) = &mut self.inner.res {
            if !self.inner.res_status.is_send_header {
                self.inner.res_status.is_chunked = res.headers().is_chunk();
                res.encode_header(&mut self.write_buf)?;
                self.inner.res_status.is_send_header = true;
            }

            if !res.body().is_end() || !self.inner.res_status.is_send_body {
                self.inner.res_status.is_send_body = true;
                let _ = res.body_mut().poll_encode(
                    cx,
                    &mut self.write_buf,
                    self.inner.res_status.is_chunked,
                );
            }
            if res.body().is_end() {
                self.inner.res_status.is_send_end = true;
                self.inner.deal_req += 1;
            }
        }

        if let Some(req) = &mut self.inner.req {
            if !self.inner.req_status.is_send_header {
                req.encode_header(&mut self.write_buf)?;
                self.inner.req_status.is_send_header = true;
            }

            if !req.body().is_end() || !self.inner.req_status.is_send_body {
                self.inner.req_status.is_send_body = true;
                let _ = req.body_mut().poll_encode(
                    cx,
                    &mut self.write_buf,
                    self.inner.req_status.is_chunked,
                );
            }
            if req.body().is_end() {
                self.inner.req_status.is_send_end = true;
                self.inner.deal_req += 1;
            }
        }

        if self.inner.req_status.is_send_end && self.inner.res_status.is_send_end {
            self.set_now_end();
        }

        if self.write_buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        println!("aaaaaaaaaaaaaaa {:?}", std::time::Instant::now());
        match ready!(Pin::new(&mut self.io).poll_write(cx, &self.write_buf.chunk()))? {
            n => {
                println!("bbbbbbbbbbbbbbbbbb = {} {:?}", n, std::time::Instant::now());
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

    fn receive_body_len(status: &mut SendStatus, body_len: usize) -> bool {
        // println!(
        //     "left len = {}, reciver = {}",
        //     status.left_body_len, body_len
        // );
        if status.left_body_len <= body_len {
            status.left_body_len = 0;
            true
        } else {
            status.left_body_len -= body_len;
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
            n @ _ => {
                println!("poll request read = {:?}", n);
                if self.inner.req_status.is_build_header {
                    self.do_deal_body(true)?;
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
                self.inner.req_status.is_send_body = false;
                self.inner.req_status.is_send_end = false;
                self.inner.req_status.is_build_header = true;
                self.inner.is_keep_alive = request.is_keep_alive();
                let body_len = request.get_body_len();
                self.inner.req_status.left_body_len = if body_len < 0 {
                    usize::MAX
                } else {
                    body_len as usize
                };
                if !request.method().is_nobody() && body_len == 0 {
                    self.inner.req_status.left_body_len = usize::MAX;
                    if request.headers().is_chunk() {
                        self.inner.req_status.is_chunked = true;
                    }
                }
                println!("body len = {:?}", self.inner.req_status.left_body_len);

                let (recv, sender) =
                    Self::build_recv_stream(&mut self.inner.res_status, &mut self.read_buf)?;
                self.inner.read_sender = sender;
                return Poll::Ready(Some(Ok(request.into(recv).0)));
            }
        }
    }

    pub fn do_deal_body(&mut self, is_req: bool) -> ProtResult<bool> {
        // chunk 格式数据
        let mut all_is_end = false;
        let is_chunked = if is_req {
            self.inner.req_status.is_chunked
        } else {
            self.inner.res_status.is_chunked
        };
        if is_chunked {
            if let Some(sender) = &self.inner.read_sender {
                loop {
                    match sender.try_reserve() {
                        Ok(p) => {
                            match Helper::parse_chunk_data(&mut self.read_buf.clone()) {
                                Ok((data, n, is_end)) => {
                                    self.read_buf.advance(n);
                                    p.send((is_end, Binary::from(data)));
                                    all_is_end = is_end
                                }
                                Err(WebError::Http(HttpError::Partial)) => return Ok(false),
                                Err(err) => return Err(err.into()),
                            }
                        }
                        Err(_) => return Err(ProtError::Extension("sender error")),
                    }
                }
            }
        } else {
            if let Some(sender) = self.inner.read_sender.take() {
                if let Ok(p) = sender.try_reserve() {
                    let binary = Binary::from(self.read_buf.chunk().to_vec());
                    let is_end = if is_req {
                        Self::receive_body_len(&mut self.inner.req_status, binary.len())
                    } else {
                        Self::receive_body_len(&mut self.inner.res_status, binary.len())
                    };
                    p.send((is_end, binary));
                    self.read_buf.advance_all();
                    self.read_buf.clear();
                    all_is_end = is_end
                }
                self.inner.read_sender = Some(sender);
            }
        };

        if all_is_end {
            self.set_now_end();
        }

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
        let _n = self.poll_write(cx)?;
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
                println!("read size = {}", n);
                if n == 0 {
                    self.inner.is_delay_close = true;
                }
                if self.inner.res_status.is_build_header {
                    let is_close = self.do_deal_body(false)?;
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
                self.inner.res_status.is_send_body = false;
                self.inner.res_status.is_send_end = false;
                self.inner.res_status.is_build_header = true;
                // self.inner.res_status.is_keep_alive = response.is_keep_alive();
                let body_len = response.get_body_len();
                self.inner.res_status.left_body_len = if body_len < 0 {
                    usize::MAX
                } else {
                    body_len as usize
                };
                if response.status().is_success() && body_len == 0 {
                    self.inner.res_status.left_body_len = usize::MAX;
                    if response.headers().is_chunk() {
                        self.inner.res_status.is_chunked = true;
                    }
                }
                println!("body len = {:?}", self.inner.res_status.left_body_len);

                let (recv, sender) =
                    Self::build_recv_stream(&mut self.inner.res_status, &mut self.read_buf)?;
                self.inner.read_sender = sender;
                return Poll::Ready(Some(Ok(response.into(recv).0)));
            }
        }
    }

    fn build_recv_stream(
        status: &mut SendStatus,
        read_buf: &mut BinaryMut,
    ) -> ProtResult<(RecvStream, Option<Sender<(bool, Binary)>>)> {
        if status.left_body_len == 0 {
            return Ok((RecvStream::empty(), None));
        } else if status.is_chunked {
            let mut binary = BinaryMut::new();
            let mut is_all_end = false;
            loop {
                match Helper::parse_chunk_data(&mut read_buf.clone()) {
                    Ok((data, n, is_end)) => {
                        binary.put_slice(&data);
                        read_buf.advance(n);
                        is_all_end = is_end;
                    }
                    Err(WebError::Http(HttpError::Partial)) => break,
                    Err(err) => return Err(err.into()),
                }
            }
            let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
            return Ok((RecvStream::new(receiver, binary, is_all_end), Some(sender)));
        } else {
            let new_response = {
                let mut binary = BinaryMut::new();
                let is_end = if status.left_body_len > read_buf.remaining() {
                    binary.put_slice(read_buf.chunk());
                    read_buf.advance_all();
                    read_buf.clear();
                    Self::receive_body_len(status, binary.remaining())
                } else {
                    binary.put_slice(&read_buf.chunk()[0..status.left_body_len]);
                    read_buf.advance(status.left_body_len);
                    Self::receive_body_len(status, status.left_body_len)
                };
                let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
                return Ok((RecvStream::new(receiver, binary, is_end), Some(sender)));
            };
        }
    }

    fn client_build_reponse(
        &mut self,
        response: Response<()>,
        binary: BinaryMut,
        is_end: bool,
    ) -> Response<RecvStream> {
        let (sender, receiver) = tokio::sync::mpsc::channel::<(bool, Binary)>(30);
        self.inner.read_sender = Some(sender);
        if is_end {
            self.set_now_end();
        }
        response.into(RecvStream::new(receiver, binary, is_end)).0
    }

    fn set_now_end(&mut self) {
        self.inner.res = None;
        self.inner.req = None;
        self.inner.req_status.clear();
        self.inner.res_status.clear();
    }

    pub fn into(self) -> (T, BinaryMut, BinaryMut) {
        (self.io, self.read_buf, self.write_buf)
    }

    pub async fn send_response(&mut self, res: Response<RecvStream>) -> ProtResult<()> {
        self.inner.res = Some(res);
        self.inner.res_status.is_send_end = false;
        Ok(())
    }

    pub async fn send_request(&mut self, req: Request<RecvStream>) -> ProtResult<()> {
        self.inner.req = Some(req);
        self.inner.req_status.is_send_end = false;
        Ok(())
    }
}
