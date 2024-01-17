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

use webparse::http2::WindowSize;

#[derive(Debug)]
#[allow(dead_code)]
pub struct FlowControl {
    window_size: i32,
    available: i32,
}

impl FlowControl {
    pub fn new(default: WindowSize) -> Self {
        Self {
            window_size: default as i32,
            available: default as i32,
        }
    }

    pub fn is_available(&self) -> bool {
        self.available > 0
    }
}
