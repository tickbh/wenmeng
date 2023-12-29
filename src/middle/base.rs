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
// Created Date: 2023/11/21 10:18:29

use crate::{Middleware, HeaderHelper};

use async_trait::async_trait;

use crate::{ProtResult, RecvRequest, RecvResponse};

pub struct BaseMiddleware {
    is_client: bool,
}

impl BaseMiddleware {
    pub fn new(is_client: bool) -> Self {
        Self { is_client }
    }
}

#[async_trait]
impl Middleware for BaseMiddleware {
    async fn process_request(&mut self, request: &mut RecvRequest) -> ProtResult<Option<RecvResponse>> {
        HeaderHelper::process_request_header(request.version(), self.is_client, request)?;
        Ok(None)
    }
    async fn process_response(
        &mut self,
        _request: &mut RecvRequest,
        response: &mut RecvResponse,
    ) -> ProtResult<()> {
        HeaderHelper::process_response_header(response.version(), self.is_client, response)?;
        Ok(())
    }
}
