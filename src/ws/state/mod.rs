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

mod state_handshake;
  
pub use state_handshake::{WsStateHandshake};
use webparse::ws::CloseData;


#[derive(Debug)]
pub enum WsState {
    /// Currently open in a sane state
    Open,

    /// The codec must be flushed
    Closing(CloseData),

    /// In a closed state
    Closed(CloseData),
}

impl WsState {
    pub fn set_closing(&mut self, data: CloseData) {
        match self {
            WsState::Open => {
                *self = WsState::Closing(data);
            },
            _ => {}
        }
    }
    pub fn set_closed(&mut self, data: Option<CloseData>) {
        match self {
            WsState::Open => {
                *self = WsState::Closed(data.unwrap_or(CloseData::normal()));
            },
            WsState::Closing(data) => {
                *self = WsState::Closed(data.clone());
            },
            _ => {}
        }
    }
}
