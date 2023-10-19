use std::{
    any::{Any, TypeId},
    net::SocketAddr,
};

use futures_core::{Future};

use webparse::{Request, Response, Serialize, HeaderName};

use crate::{ProtResult, RecvStream, HeaderHelper};


pub struct HttpHelper;

impl HttpHelper {

    pub async fn handle_request<F, Fut, Res, Req>(
        addr: &Option<SocketAddr>,
        mut r: Request<RecvStream>,
        f: &mut F,
    ) -> ProtResult<Response<RecvStream>>
    where
        F: FnMut(Request<Req>) -> Fut,
        Fut: Future<Output = ProtResult<Response<Res>>>,
        Req: From<RecvStream>,
        Req: Serialize + Any,
        RecvStream: From<Res>,
        Res: Serialize + Any,
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
        
        if TypeId::of::<Req>() != TypeId::of::<RecvStream>() {
            let _ = r.body_mut().wait_all().await;
        }
        if let Some(addr) = addr {
            r.headers_mut().system_insert("$client_ip".to_string(), format!("{}", addr));
        }
        match f(r.into_type::<Req>()).await {
            Ok(res) => {
                let mut res = res.into_type::<RecvStream>();
                // 如果外部有设置编码，内部不做改变，如果有body大小值，不做任何改变，因为改变会变更大小值
                if res.get_body_len() == 0 && res.headers_mut().get_option_value(&HeaderName::CONTENT_ENCODING).is_none() && (!res.body().is_end() || res.body_mut().origin_len() > 1024) {
                    if gzip {
                        res.headers_mut().insert(HeaderName::CONTENT_ENCODING, "gzip");
                    } else if br {
                        res.headers_mut().insert(HeaderName::CONTENT_ENCODING, "br");
                    } else if deflate {
                        res.headers_mut().insert(HeaderName::CONTENT_ENCODING, "deflate");
                    }
                }
                // HeaderHelper::process_response_header(&mut res)?;
                return Ok(res)
            }
            Err(_) => {
                Ok(Response::builder().status(500).body("server inner error").unwrap().into_type())
            }
        }
    }
}