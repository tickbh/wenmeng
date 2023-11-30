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
// Created Date: 2023/10/13 10:22:00

pub struct Consts;

impl Consts {
    /// 原始加密信息, 如果收到头header为brotli则+COMPRESS_METHOD_BROTLI则归为0, 则原始数据不处理
    // pub const COMPRESS_METHOD_ORIGIN_BROTLI: i8 = -3;
    // pub const COMPRESS_METHOD_ORIGIN_DEFLATE: i8 = -2;
    // pub const COMPRESS_METHOD_ORIGIN_GZIP: i8 = -1;
    pub const COMPRESS_METHOD_NONE: i8 = 0;
    pub const COMPRESS_METHOD_GZIP: i8 = 1;
    pub const COMPRESS_METHOD_DEFLATE: i8 = 2;
    pub const COMPRESS_METHOD_BROTLI: i8 = 3;
}