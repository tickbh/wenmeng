// Copyright 2022 - 2024 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2024/01/09 10:49:38

use std::io;

use crate::{
    ws::{WsHandshake, WsOption, WsTrait},
    ProtError, ProtResult, RecvRequest, RecvResponse, Server,
};
use async_trait::async_trait;
use tokio::{
    io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc::{channel, Receiver, Sender},
};
use webparse::{ws::OwnedMessage, BinaryMut, Buf, Response};

/// 将websocket的流量转化成的tcp流量
pub struct WsToStream<T: AsyncRead + AsyncWrite + Unpin + Send + 'static, A: ToSocketAddrs> {
    addr: A,
    domain: Option<String>,
    io: T,
}

struct Operate {
    domain: Option<String>,
    /// 将tcp来的数据流转发到websocket
    stream_sender: Sender<Vec<u8>>,
    /// 从websocket那接收信息
    receiver: Option<Receiver<OwnedMessage>>,
}

#[async_trait]
impl WsTrait for Operate {
    #[inline]
    async fn on_request(&mut self, req: &RecvRequest) -> ProtResult<RecvResponse> {
        if self.domain.is_some() {
            if req.get_host() != self.domain {
                Ok(Response::builder()
                    .status(400)
                    .body("host not match")?
                    .into_type())
            } else {
                WsHandshake::build_request(req)
            }
        } else {
            WsHandshake::build_request(req)
        }
        // warn!("Handler received request:\n{}", req);
    }

    async fn on_open(&mut self, _shake: WsHandshake) -> ProtResult<Option<WsOption>> {
        // 将receiver传给控制中心, 以让其用该receiver做接收
        let mut option = WsOption::new();
        if self.receiver.is_some() {
            option.set_receiver(self.receiver.take().unwrap());
        }
        Ok(Some(option))
    }

    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()> {
        println!("callback on message = {:?}", msg);
        // let _ = self.sender.send(msg).await;
        match msg {
            OwnedMessage::Text(v) => self
                .stream_sender
                .send(v.into_bytes())
                .await
                .map_err(|_| ProtError::Extension("close"))?,
            OwnedMessage::Binary(v) => self
                .stream_sender
                .send(v)
                .await
                .map_err(|_| ProtError::Extension("close"))?,
            _ => (),
        }
        Ok(())
    }

    async fn on_interval(&mut self, _option: &mut Option<WsOption>) -> ProtResult<()> {
        println!("on_interval!!!!!!!");
        Ok(())
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static, A: ToSocketAddrs> WsToStream<T, A> {
    pub fn new(io: T, addr: A) -> ProtResult<Self> {
        Ok(Self {
            addr,
            io,
            domain: None,
        })
    }

    pub fn set_domain(&mut self, domain: String) {
        self.domain = Some(domain);
    }

    pub async fn copy_bidirectional(self) -> ProtResult<()> {
        let (ws_sender, ws_receiver) = channel(10);
        let (stream_sender, stream_receiver) = channel::<Vec<u8>>(10);
        let stream = TcpStream::connect(self.addr).await?;
        let io = self.io;
        tokio::spawn(async move {
            let mut server = Server::new(io, None);
            server.set_callback_ws(Box::new(Operate {
                stream_sender,
                receiver: Some(ws_receiver),
                domain: self.domain,
            }));
            let e = server.incoming().await;
            println!("close server ==== addr = {:?} e = {:?}", 0, e);
        });
        Self::bind(stream, ws_sender, stream_receiver).await?;
        Ok(())
    }

    pub async fn bind<S>(
        io: S,
        ws_sender: Sender<OwnedMessage>,
        mut stream_receiver: Receiver<Vec<u8>>,
    ) -> ProtResult<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buf = vec![0; 20480];
        let (mut reader, mut writer) = split(io);
        let (mut read, mut write) = (BinaryMut::new(), BinaryMut::new());
        loop {
            tokio::select! {
                n = reader.read(&mut buf) => {
                    let n = n?;
                    if n == 0 {
                        return Ok(())
                    } else {
                        read.put_slice(&buf[..n]);
                    }
                },
                r = writer.write(write.chunk()), if write.has_remaining() => {
                    match r {
                        Ok(n) => {
                            write.advance(n);
                            if !write.has_remaining() {
                                write.clear();
                            }
                        }
                        Err(_) => todo!(),
                    }
                }
                r = stream_receiver.recv() => {
                    println!("stream receiver = {:?}", r);
                    if let Some(v) = r {
                        write.put_slice(&v);
                    } else {
                        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid frame").into())
                    }
                }
                p = ws_sender.reserve(), if read.has_remaining() => {
                    match p {
                        Err(_)=>{
                            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid frame").into())
                        }
                        Ok(p) => {
                            let msg = OwnedMessage::Binary(read.chunk().to_vec());
                            read.clear();
                            p.send(msg);
                        },
                    }
                }
            }
        }
        Ok(())
    }
}
