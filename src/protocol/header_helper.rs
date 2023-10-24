use webparse::{Serialize, Request, Response, HeaderName, HeaderMap, Version};

use crate::{RecvStream, ProtResult, Consts};

pub struct HeaderHelper;

impl HeaderHelper {
    pub const REQ: &'static str = "proxy";
    pub const OP_ADD: &'static str = "+";
    pub const OP_DEL: &'static str = "-";
    pub const OP_DEFAULT: &'static str = "?";
    pub fn rewrite_request<T>(request: &mut Request<T>, headers: &Vec<Vec<String>>)
    where
        T: Serialize,
    {
        for h in headers {
            if h.len() == 0 {
                continue;
            }
            if h[0] != Self::REQ {
                continue;
            }

            Self::rewrite_header(Some(request), None, &h[1..]);
        }
    }

    pub fn rewrite_response<T>(response: &mut Response<T>, headers: &Vec<Vec<String>>)
    where
        T: Serialize,
    {
        for h in headers {
            if h.len() == 0 {
                continue;
            }
            if h[0] == Self::REQ {
                continue;
            }

            Self::rewrite_header(None, Some(response), &h[0..]);
            
        }
    }

    pub fn rewrite_header<T: Serialize>(mut request: Option<&mut Request<T>>, mut response: Option<&mut Response<T>>, value: &[String]) {
        if value.len() < 2 {
            return;
        }
        match &*value[0] {
            Self::OP_ADD => {
                if value.len() < 3 {
                    return;
                }
                let v = Self::convert_value(&mut request, &mut response, value[2].to_string());
                if request.is_some() {
                    request.unwrap().headers_mut().push(value[1].to_string(), v);
                } else {
                    response.unwrap().headers_mut().push(value[1].to_string(), v);
                }
            }
            Self::OP_DEL => {
                if value.len() < 2 {
                    return;
                }if request.is_some() {
                    request.unwrap().headers_mut().remove(&value[1]);
                } else {
                    response.unwrap().headers_mut().remove(&value[1]);
                }
            }
            Self::OP_DEFAULT => {
                if value.len() < 3 {
                    return;
                }
                let contains = if request.is_some() {
                    request.as_ref().unwrap().headers().contains(&value[1])
                } else {
                    response.as_ref().unwrap().headers().contains(&value[1])
                };

                if contains {
                    return;
                }
                let v = Self::convert_value(&mut request, &mut response, value[2].to_string());
                if request.is_some() {
                    request.unwrap().headers_mut().push(value[1].to_string(), v);
                } else {
                    response.unwrap().headers_mut().push(value[1].to_string(), v);
                }
            }
            _ => {
                if value.len() < 2 {
                    return;
                }
                let v = Self::convert_value(&mut request, &mut response, value[1].to_string());
                if request.is_some() {
                    request.unwrap().headers_mut().push(value[0].to_string(), v);
                } else {
                    response.unwrap().headers_mut().push(value[0].to_string(), v);
                }
            }
        }
    }

    fn convert_value<T: Serialize>(request: &mut Option<&mut Request<T>>, response: &mut Option<&mut Response<T>>, value: String) -> String {
        if value.len() == 0 {
            return value;
        }
        if value.as_bytes()[0] == b'$' {
            if request.is_some() {
                if let Some(convert) = request.as_mut().unwrap().headers_mut().system_get(&value) {
                    println!("get {} convert {}", value, convert);
                    return convert.to_string();
                } else {
                    match &*value {
                        "$host" => {
                            return request.as_ref().unwrap().get_host().unwrap_or(String::new());
                        }
                        "$url" => {
                            println!("get {} convert {}", value, format!("{}", request.as_ref().unwrap().url()));
                            return format!("{}", request.as_ref().unwrap().url());
                        }
                        _ => {
                            return "unknown".to_string();
                        }
                    }
                }
            } else {
                if let Some(convert) = response.as_mut().unwrap().headers_mut().system_get(&value) {
                    println!("get {} convert {}", value, convert);
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

    pub fn process_headers(version: Version, headers: &mut HeaderMap, body: &mut RecvStream) -> ProtResult<()> {
        let compress = Self::get_compress_method(headers);
        let compress = body.add_compress_method(compress);

        let is_chunked = headers.is_chunked();
        body.set_chunked(is_chunked);
        let header_body_len = headers.get_body_len();
        if version.is_http2() {
            headers.remove(&HeaderName::TRANSFER_ENCODING);
            headers.remove(&HeaderName::CONNECTION);
            headers.remove(&"Keep-Alive");
        }
        if compress == Consts::COMPRESS_METHOD_NONE {
            if !is_chunked && header_body_len == 0 && body.is_end() {
                body.process_data(None)?;
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
                        body.process_data(None)?;
                        let len = body.body_len();
                        headers.insert(HeaderName::CONTENT_LENGTH, len);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn process_request_header(version: Version, req: &mut Request<RecvStream>) -> ProtResult<()> {
        let (h, b) = req.headers_body_mut();
        Self::process_headers(version, h, b)?;
        Ok(())
    }

    pub fn process_response_header(version: Version, res: &mut Response<RecvStream>) -> ProtResult<()> {
        let (h, b) = res.headers_body_mut();
        Self::process_headers(version, h, b)?;
        Ok(())
    }

}
