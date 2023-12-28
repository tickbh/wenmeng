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
// Created Date: 2023/08/31 10:06:46

mod protocol;

pub use protocol::*;
use webparse::{Request, Response};

pub type RecvRequest = Request<Body>;
pub type RecvResponse = Response<Body>;

use async_trait::async_trait;

#[async_trait]
pub trait OperateTrait {
    /// 处理请求并返回正确的数据
    async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse>;
    
    /// 处理中间件的请求，跟中间件相关的处理
    async fn middle_operate(&mut self, req: &mut RecvRequest, middles: &mut Vec<Box<dyn Middleware>>) -> ProtResult<()> {
        let _req = req;
        let _middle = middles;
        Ok(())
    }

    /// 是否主动结束服务，返回false则表示服务暂停
    fn is_continue_next(&self) -> bool {
        true
    }
}