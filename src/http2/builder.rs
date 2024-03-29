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

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::http::http2::frame::Settings;

use crate::ServerH2Connection;

use super::ClientH2Connection;

#[derive(Clone, Debug)]
pub struct Builder {
    /// Time to keep locally reset streams around before reaping.
    pub reset_stream_duration: Duration,

    /// Maximum number of locally reset streams to keep at a time.
    pub reset_stream_max: usize,

    /// Maximum number of remotely reset streams to allow in the pending
    /// accept queue.
    pub pending_accept_reset_stream_max: usize,

    /// Initial `Settings` frame to send as part of the handshake.
    pub settings: Settings,

    /// Initial target window size for new connections.
    pub initial_target_connection_window_size: Option<u32>,

    /// Maximum amount of bytes to "buffer" for writing per stream.
    pub max_send_buffer_size: usize,
}

impl Builder {
    pub fn new() -> Builder {
        use webparse::http::http2::*;
        Builder {
            reset_stream_duration: Duration::from_secs(DEFAULT_RESET_STREAM_SECS),
            reset_stream_max: DEFAULT_RESET_STREAM_MAX,
            pending_accept_reset_stream_max: DEFAULT_REMOTE_RESET_STREAM_MAX,
            settings: Settings::default(),
            initial_target_connection_window_size: None,
            max_send_buffer_size: DEFAULT_MAX_SEND_BUFFER_SIZE,
        }
    }

    pub fn initial_window_size(mut self, size: u32) -> Self {
        self.settings.set_initial_window_size(Some(size));
        self
    }

    pub fn initial_connection_window_size(mut self, size: u32) -> Self {
        self.initial_target_connection_window_size = Some(size);
        self
    }

    pub fn max_frame_size(mut self, max: u32) -> Self {
        self.settings.set_max_frame_size(Some(max));
        self
    }

    pub fn max_header_list_size(mut self, max: u32) -> Self {
        self.settings.set_max_header_list_size(Some(max));
        self
    }

    pub fn max_concurrent_streams(mut self, max: u32) -> Self {
        self.settings.set_max_concurrent_streams(Some(max));
        self
    }

    pub fn set_enable_push(mut self, enable: bool) -> Self {
        self.settings.set_enable_push(enable);
        self
    }

    pub fn max_concurrent_reset_streams(mut self, max: usize) -> Self {
        self.reset_stream_max = max;
        self
    }

    pub fn max_pending_accept_reset_streams(mut self, max: usize) -> Self {
        self.pending_accept_reset_stream_max = max;
        self
    }

    pub fn max_send_buffer_size(mut self, max: usize) -> Self {
        assert!(max <= std::u32::MAX as usize);
        self.max_send_buffer_size = max;
        self
    }

    pub fn reset_stream_duration(mut self, dur: Duration) -> Self {
        self.reset_stream_duration = dur;
        self
    }

    pub fn enable_connect_protocol(mut self) -> Self {
        self.settings.set_enable_connect_protocol(Some(1));
        self
    }

    pub fn server_connection<T>(self, io: T) -> ServerH2Connection<T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ServerH2Connection::new(io, self)
    }

    pub fn client_connection<T>(self, io: T) -> ClientH2Connection<T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        ClientH2Connection::new(io, self)
    }
}
