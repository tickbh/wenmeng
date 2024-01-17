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

use std::{io};

use async_trait::async_trait;
use tokio::{
    io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::mpsc::{channel, Receiver, Sender},
};
use webparse::{ws::OwnedMessage, BinaryMut, Buf, Url, WebError};
use crate::{
    ws::{WsHandshake, WsOption, WsTrait},
    Client, ProtError, ProtResult,
};

/// 将tcp的流量转化成websocket的流量
pub struct StreamToWs<T: AsyncRead + AsyncWrite + Unpin> {
    url: Url,
    domain: Option<String>,
    io: T,
}

struct Operate {
    /// 将tcp来的数据流转发到websocket
    stream_sender: Sender<Vec<u8>>,
    /// 从websocket那接收信息
    receiver: Option<Receiver<OwnedMessage>>,
}

#[async_trait]
impl WsTrait for Operate {
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

impl<T: AsyncRead + AsyncWrite + Unpin> StreamToWs<T> {
    pub fn new<U>(io: T, url: U) -> ProtResult<Self>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<WebError>,
    {
        let url = Url::try_from(url).map_err(Into::into)?;
        Ok(Self { url, io, domain: None })
    }

    pub async fn copy_bidirectional(self) -> ProtResult<()> {
        let (ws_sender, ws_receiver) = channel::<OwnedMessage>(10);
        let (stream_sender, stream_receiver) = channel::<Vec<u8>>(10);
        let url = self.url;
        tokio::spawn(async move {
            let build = if let Some(d) = &self.domain {
                Client::builder().url(url).unwrap().connect_with_domain(&d).await
            } else {
                Client::builder().url(url).unwrap().connect().await
            };
            if let Ok(mut client) = build {
                client.set_callback_ws(Box::new(Operate {
                    stream_sender,
                    receiver: Some(ws_receiver),
                }));
                let _e = client.wait_ws_operate().await;
            }
        });
        Self::bind(self.io, ws_sender, stream_receiver).await?;
        Ok(())
    }

    pub fn set_domain(&mut self, domain: String) {
        self.domain = Some(domain);
    }

    pub async fn bind(
        io: T,
        ws_sender: Sender<OwnedMessage>,
        mut stream_receiver: Receiver<Vec<u8>>,
    ) -> ProtResult<()> {
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
    }
}
