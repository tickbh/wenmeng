use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::http::http2::frame::{Frame, Settings};

use crate::{proto::http2::codec::Codec, ProtoResult};

pub struct StateSettings {
    state: LocalState,
    remote: Option<Settings>,
}

enum LocalState {
    /// 未初始化
    None,
    /// 设置发送的settings
    Send(Settings),
    /// 设置等待确认settings
    WaitAck(Settings),
    /// 发送并收到了设置
    Done,
}

impl StateSettings {
    pub fn new() -> Self {
        StateSettings {
            state: LocalState::Send(Settings::default()),
            remote: None,
        }
    }

    pub fn pull_handle<T>(
        &mut self,
        cx: &mut Context<'_>,
        codec: &mut Codec<T>,
    ) -> Poll<ProtoResult<()>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {

        match &self.state {
            LocalState::None => return Poll::Ready(Ok(())),
            LocalState::Send(settings) => {
                codec.send_frame(Frame::Settings(settings.clone()))?;
                self.state = LocalState::WaitAck(settings.clone());
                return Poll::Ready(Ok(()))
            },
            LocalState::WaitAck(_) => {
                return Poll::Ready(Ok(()))
            },
            LocalState::Done => (),
        };

        if let Some(settings) = &self.remote {
            if !codec.poll_ready(cx)?.is_ready() {
                return Poll::Pending;
            }
            let frame = Settings::ack();
            codec.send_frame(Frame::Settings(frame))?;
        }

        self.remote = None;

        // loop {
        //     match &mut self.state {
        //         LocalState::None => {
        //             self.state = Handshaking::Flushing(Flush(Binary::new()));
        //         }
        //         Handshaking::Flushing(flush) => {
        //             match ready!(flush.pull_handle(cx, codec)) {
        //                 Ok(_) => {
        //                     tracing::trace!(flush.poll = %"Ready");
        //                     self.state = Handshaking::ReadingPreface(ReadPreface::new());
        //                     continue;
        //                 }
        //                 Err(e) => return Poll::Ready(Err(e)),
        //             };
        //         }
        //         Handshaking::ReadingPreface(read) => {
        //             match ready!(read.pull_handle(cx, codec)) {
        //                 Ok(_) => {
        //                     tracing::trace!(flush.poll = %"Ready");
        //                     self.state = Handshaking::Done;
        //                     return Poll::Ready(Ok(()));
        //                 }
        //                 Err(e) => return Poll::Ready(Err(e)),
        //             };
        //         }
        //         Handshaking::Done => {
        //             return Poll::Ready(Ok(()));
        //         }
        //     }
        // }
        return Poll::Ready(Ok(()))
    }

    pub fn recv_setting(&mut self, setting: Settings) -> ProtoResult<()> {
        if setting.is_ack() {
            self.state = LocalState::Done;
            Ok(())
        } else {
            self.remote = Some(setting);
            Ok(())
        }
    }
}
