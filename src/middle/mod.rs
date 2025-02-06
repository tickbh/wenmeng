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
// Created Date: 2023/11/20 02:17:22

use async_trait::async_trait;

use crate::{RecvRequest, ProtResult, RecvResponse, ProtError};


#[async_trait]
pub trait Middleware: Send + Sync {
    async fn process_request(&mut self, request: &mut RecvRequest) -> ProtResult<Option<RecvResponse>>;
    async fn process_response(&mut self, response: &mut RecvResponse) -> ProtResult<()>;
    async fn process_error(&mut self, _request: Option<&mut RecvRequest>, _error: &ProtError) {}
}

mod base;

pub use base::BaseMiddleware;