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
// Created Date: 2023/10/16 09:44:12

use std::net::SocketAddr;

use webparse::{HeaderName, Response, Version};

use crate::{Middleware, OperateTrait, ProtResult, RecvRequest, RecvResponse};

pub struct HttpHelper;

impl HttpHelper {
    pub async fn handle_request<F>(
        version: Version,
        addr: &Option<SocketAddr>,
        mut r: RecvRequest,
        f: &mut F,
        middles: &mut Vec<Box<dyn Middleware>>,
    ) -> ProtResult<RecvResponse>
    where
        F: OperateTrait + Send,
    {
        let (mut gzip, mut deflate, mut br) = (false, false, false);
        if let Some(accept) = r.headers().get_option_value(&HeaderName::ACCEPT_ENCODING) {
            if accept.contains("gzip".as_bytes()) {
                gzip = true;
            }
            if accept.contains("deflate".as_bytes()) {
                deflate = true;
            }
            if accept.contains("br".as_bytes()) {
                br = true;
            }
        }

        if let Some(addr) = addr {
            r.headers_mut()
                .system_insert("{client_ip}".to_string(), format!("{}", addr.ip()));
            r.headers_mut()
                .system_insert("{client_addr}".to_string(), format!("{}", addr));
        }
        let mut response = None;

        f.middle_operate(&mut r, middles).await?;

        for middle in middles.iter_mut() {
            if let Some(res) = middle.as_mut().process_request(&mut r).await? {
                response = Some(res);
                break;
            }
        }

        if response.is_none() {
            let res = match f.operate(&mut r).await {
                Ok(mut res) => {
                    *res.version_mut() = version;
                    // 如果外部有设置编码，内部不做改变，如果有body大小值，不做任何改变，因为改变会变更大小值
                    if res.get_body_len() == 0
                        && res
                            .headers_mut()
                            .get_option_value(&HeaderName::CONTENT_ENCODING)
                            .is_none()
                        && (!res.body().is_end() || res.body_mut().origin_len() > 1024)
                    {
                        if gzip {
                            res.headers_mut()
                                .insert(HeaderName::CONTENT_ENCODING, "gzip");
                        } else if br {
                            res.headers_mut().insert(HeaderName::CONTENT_ENCODING, "br");
                        } else if deflate {
                            res.headers_mut()
                                .insert(HeaderName::CONTENT_ENCODING, "deflate");
                        }
                    }
                    // HeaderHelper::process_response_header(&mut res)?;
                    res
                }
                Err(e) => {
                    log::info!("处理数据时出错:{:?}", e);
                    for i in 0usize..middles.len() {
                        middles[i].process_error(Some(&mut r), &e).await;
                    }
                    Response::builder()
                        .status(500)
                        .body("server inner error")
                        .unwrap()
                        .into_type()
                }
            };
            response = Some(res);
        }
        let mut response = response.unwrap();
        for i in (0usize..middles.len()).rev() {
            middles[i].process_response(&mut r, &mut response).await?;
        }
        Ok(response)
    }
}
