use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio_stream::StreamExt;

use futures_core::{ready, stream, Stream};
use tokio::{io::{AsyncRead, AsyncWrite}, sync::mpsc::Sender};
use webparse::{
    http::{
        http2::frame::{Flag, Frame, Kind, Settings, StreamIdentifier},
        request,
    },
    Binary, BinaryMut, Request, Response,
};

use crate::ProtoResult;

use super::{
    codec::Codec, inner_stream::InnerStream, send_response::SendControl, state::StateHandshake,
    PriorityQueue, RecvStream, SendResponse, StateSettings, WindowSize,
};

#[derive(Debug, Clone)]
pub struct ControlConfig {
    pub next_stream_id: StreamIdentifier,
    pub initial_max_send_streams: usize,
    pub max_send_buffer_size: usize,
    pub reset_stream_duration: Duration,
    pub reset_stream_max: usize,
    pub remote_reset_stream_max: usize,
    pub settings: Settings,
}

impl ControlConfig {
    pub fn apply_remote_settings(&mut self, settings: &Settings) {}
}

pub struct Control {
    /// 所有收到的帧, 如果收到Header结束就开始返回request, 后续收到Data再继续返回直至结束,
    /// id为0的帧为控制帧, 需要立即做处理
    recv_frames: HashMap<StreamIdentifier, InnerStream>,
    send_frames: PriorityQueue,
    reponse_queue: Arc<Mutex<Vec<SendResponse>>>,

    handshake: StateHandshake,
    setting: StateSettings,
    config: ControlConfig,
    
    write_sender: Sender<()>,
}

impl Control {
    pub fn new(config: ControlConfig, write_sender: Sender<()>) -> Self {
        Control {
            recv_frames: HashMap::new(),
            send_frames: PriorityQueue::new(),
            reponse_queue: Arc::new(Mutex::new(Vec::new())),
            setting: StateSettings::new(config.settings.clone()),
            handshake: StateHandshake::new_server(),
            config,
            write_sender,
        }
    }

    pub fn encode_response<T>(&mut self, codec: &mut Codec<T>) -> ProtoResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut list = self.reponse_queue.lock().unwrap();
        let vals = (*list).drain(..).collect::<Vec<SendResponse>>();
        for mut l in vals {
            let (isend, vec) = l.encode_frames();
            self.send_frames.send_frames(l.stream_id, vec)?;
            if !isend {
                (*list).push(l);
            }
        }
        Ok(())
    }

    pub fn poll_write<T>(&mut self, cx: &mut Context, codec: &mut Codec<T>) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        self.encode_response(codec)?;
        match ready!(self.send_frames.poll_handle(cx, codec)) {
            Some(Err(e)) => return Poll::Ready(Err(e)),
            _ => (),
        }
        ready!(codec.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    pub fn poll_request<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<Option<ProtoResult<(Request<RecvStream>, SendControl)>>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ready!(self.handshake.poll_handle(cx, codec))?;
        loop {

            println!("aaaaaaaaaaaaaaa");
            ready!(self.setting.poll_handle(cx, codec, &mut self.config))?;
            // 写入如果pending不直接pending, 等尝试读pending则返回
            match self.poll_write(cx, codec) {
                Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                _ => (),
            }
            let xxx = Pin::new(&mut *codec).poll_next(cx);
            println!("xxxx = {:?}", xxx.is_pending());
            match ready!(xxx) {
                Some(Ok(frame)) => {
                    let mut bytes = BinaryMut::new();
                    match &frame {
                        Frame::Settings(settings) => {
                            self.setting
                                .recv_setting(codec, settings.clone(), &mut self.config)?;
                        }
                        Frame::Data(_) => {}
                        Frame::Headers(header) => match self.recv_frame(frame) {
                            None => {
                                continue;
                            }
                            Some(Err(e)) => {
                                return Poll::Ready(Some(Err(e)));
                            }
                            Some(Ok(r)) => {
                                return Poll::Ready(Some(Ok(r)));
                            }
                        },
                        Frame::Priority(_) => {}
                        Frame::PushPromise(_) => {}
                        Frame::Ping(_) => {}
                        Frame::GoAway(_) => {}
                        Frame::WindowUpdate(_) => {}
                        Frame::Reset(_) => {}
                    }
                }
                Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                None => return Poll::Ready(None),
            }
        }
    }

    pub fn build_request(
        &mut self,
        frames: &Vec<Frame<Binary>>,
    ) -> Option<ProtoResult<Request<Binary>>> {
        None
    }

    pub fn recv_frame(
        &mut self,
        frame: Frame<Binary>,
    ) -> Option<ProtoResult<(Request<RecvStream>, SendControl)>> {
        let stream_id = frame.stream_id();
        if stream_id.is_zero() {
            return None;
        }

        let is_end_headers = frame.is_end_headers();
        let is_end_stream = frame.is_end_stream();

        if !self.recv_frames.contains_key(&stream_id) {
            self.recv_frames.insert(stream_id, InnerStream::new(frame));
        } else {
            self.recv_frames.get_mut(&stream_id).unwrap().push(frame);
        }

        if is_end_headers {
            match self
                .recv_frames
                .get_mut(&stream_id)
                .unwrap()
                .build_request()
            {
                Err(e) => return Some(Err(e)),
                Ok(r) => {
                    let method = r.method().clone();
                    Some(Ok((
                    r,
                    SendControl::new(stream_id, self.reponse_queue.clone(), method, self.write_sender.clone()),
                )))},
            }
        } else {
            None
        }
    }
}
