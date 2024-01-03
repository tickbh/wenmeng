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
// Created Date: 2024/01/02 10:51:49

use webparse::Binary;

use super::state::{WsStateGoAway, WsStateHandshake, WsStatePingPong};

pub(crate) struct Control {
    hand_shake: WsStateHandshake,
    goaway: WsStateGoAway,
    pingpong: WsStatePingPong,
}

impl Control {
    pub fn new() -> Self {
        Self {
            hand_shake: WsStateHandshake::new_server(),
            goaway: WsStateGoAway::new(),
            pingpong: WsStatePingPong::new(),
        }
    }

    
    pub fn set_handshake_status(&mut self, binary: Binary, is_client: bool) {
        // self.is_client = is_client;
        // self.state = Handshaking::Flushing(Flush(binary))
    }
}
