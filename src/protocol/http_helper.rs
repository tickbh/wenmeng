use std::{
    net::SocketAddr,
};



use webparse::{HeaderName, Response};

use crate::{ProtResult, RecvRequest, RecvResponse, RecvStream, OperateTrait, Middleware};

pub struct HttpHelper;

impl HttpHelper {
    pub async fn handle_request<F>(
        addr: &Option<SocketAddr>,
        mut r: RecvRequest,
        f: &mut F,
        middles: &mut Vec<Box<dyn Middleware>>
    ) -> ProtResult<RecvResponse>
    where
        F: OperateTrait,
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
                .system_insert("{client_ip}".to_string(), format!("{}", addr));
        }
        
        for middle in middles.iter_mut() {
            middle.as_mut().process_request(&mut r).await?;
        }

        let mut res = match f.operate(&mut r).await {
            Ok(res) => {
                let res = res.into_type::<RecvStream>();
                // 如果外部有设置编码，内部不做改变，如果有body大小值，不做任何改变，因为改变会变更大小值
                // if res.get_body_len() == 0 && res.headers_mut().get_option_value(&HeaderName::CONTENT_ENCODING).is_none() && (!res.body().is_end() || res.body_mut().origin_len() > 1024) {
                //     if gzip {
                //         res.headers_mut().insert(HeaderName::CONTENT_ENCODING, "gzip");
                //     } else if br {
                //         res.headers_mut().insert(HeaderName::CONTENT_ENCODING, "br");
                //     } else if deflate {
                //         res.headers_mut().insert(HeaderName::CONTENT_ENCODING, "deflate");
                //     }
                // }
                // HeaderHelper::process_response_header(&mut res)?;
                res
            }
            Err(e) => {
                log::info!("处理数据时出错:{:?}", e);
                Response::builder()
                    .status(500)
                    .body("server inner error")
                    .unwrap()
                    .into_type()
            }
        };
        
        for middle in middles.iter_mut() {
            middle.process_response(&mut r, &mut res).await?;
        }

        Ok(res)
    }
}
