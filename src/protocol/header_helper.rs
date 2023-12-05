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
// Created Date: 2023/10/09 08:30:28

use webparse::{Serialize, Request, Response, HeaderName, HeaderMap, Version};

use crate::{RecvStream, ProtResult, Consts, RecvResponse, RecvRequest};

pub struct HeaderHelper;

impl HeaderHelper {
    pub fn convert_value<T: Serialize>(request: &mut Option<&mut Request<T>>, response: &mut Option<&mut Response<T>>, value: String) -> String {
        if value.len() == 0 {
            return value;
        }
        if value.as_bytes()[0] == b'{' {
            if request.is_some() {
                if let Some(convert) = request.as_mut().unwrap().headers_mut().system_get(&value) {
                    return convert.to_string();
                } else {
                    match &*value {
                        "{host}" => {
                            return request.as_ref().unwrap().get_host().unwrap_or(String::new());
                        }
                        "{url}" => {
                            return format!("{}", request.as_ref().unwrap().url());
                        }
                        _ => {
                            return "unknown".to_string();
                        }
                    }
                }
            } else {
                if let Some(convert) = response.as_mut().unwrap().headers_mut().system_get(&value) {
                    return convert.to_string();
                } else {
                    match &*value {
                        _ => {
                            return "unknown".to_string();
                        }
                    }
                }
            }
        }
        return value;
    }

    pub fn get_compress_method(header: &HeaderMap) -> i8 {
        if let Some(value) = header.get_option_value(&HeaderName::CONTENT_ENCODING) {
            if value.contains(b"gzip") {
                return Consts::COMPRESS_METHOD_GZIP;
            } else if value.contains(b"deflate") {
                return Consts::COMPRESS_METHOD_DEFLATE;
            } else if value.contains(b"br") {
                return Consts::COMPRESS_METHOD_BROTLI;
            }
        };
        return Consts::COMPRESS_METHOD_NONE;
    }

    pub fn process_headers(version: Version, is_client: bool, headers: &mut HeaderMap, body: &mut RecvStream) -> ProtResult<()> {
        let compress = Self::get_compress_method(headers);
        if version.is_http2() {
            headers.remove(&HeaderName::TRANSFER_ENCODING);
            headers.remove(&HeaderName::CONNECTION);
            headers.remove(&"Keep-Alive");
        }
        let is_chunked = headers.is_chunked();
        let compress = if is_client {
            body.set_origin_compress_method(compress)
        } else {
            body.set_chunked(is_chunked);
            body.add_compress_method(compress)
        };

        let header_body_len = headers.get_body_len();
        if compress == Consts::COMPRESS_METHOD_NONE {
            if !is_chunked && header_body_len == 0 && body.is_end() {
                let _ = body.process_data(None)?;
                let len = body.body_len();
                headers.insert(HeaderName::CONTENT_LENGTH, len);
            }
        } else {
            if header_body_len == 0 {
                // 非完整数据，无法立马得到最终数据，写入chunked
                if !body.is_end() {
                    if !is_chunked {
                        if version.is_http1() {
                            headers.insert(HeaderName::TRANSFER_ENCODING, "chunked");
                        }
                    }
                } else {
                    if !is_chunked {
                        let _ = body.process_data(None)?;
                        let len = body.body_len();
                        headers.insert(HeaderName::CONTENT_LENGTH, len);
                    } else {
                        let _ = body.process_data(None)?;
                        // let len = body.body_len();
                        // headers.insert(HeaderName::CONTENT_LENGTH, len);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn process_request_header(version: Version, is_client: bool, req: &mut RecvRequest) -> ProtResult<()> {
        let (h, b) = req.headers_body_mut();
        Self::process_headers(version, is_client, h, b)?;
        Ok(())
    }

    pub fn process_response_header(version: Version, is_client: bool, res: &mut RecvResponse) -> ProtResult<()> {
        let (h, b) = res.headers_body_mut();
        Self::process_headers(version, is_client, h, b)?;
        Ok(())
    }

}
