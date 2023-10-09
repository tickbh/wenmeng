use webparse::{Serialize, Request, HeaderMap, Response};

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

            Self::rewrite_header(request.headers_mut(), &h[1..]);
            
        }
    }

    pub fn rewrite_response<T>(request: &mut Response<T>, headers: &Vec<Vec<String>>)
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

            Self::rewrite_header(request.headers_mut(), &h[0..]);
            
        }
    }

    pub fn rewrite_header(header: &mut HeaderMap, value: &[String]) {
        if value.len() < 2 {
            return;
        }
        match &*value[0] {
            Self::OP_ADD => {
                if value.len() < 3 {
                    return;
                }
                let v = Self::convert_value(header, value[2].to_string());
                header.push(value[1].to_string(), v);
            }
            Self::OP_DEL => {
                if value.len() < 2 {
                    return;
                }
                header.remove(&value[1]);
            }
            Self::OP_DEFAULT => {
                if value.len() < 3 {
                    return;
                }
                if header.contains(&value[1]) {
                    return;
                }
                let v = Self::convert_value(header, value[2].to_string());
                header.insert(value[1].to_string(), v);
            }
            _ => {
                if value.len() < 2 {
                    return;
                }
                let v = Self::convert_value(header, value[1].to_string());
                header.insert(value[0].to_string(), v);
            }
        }
    }

    fn convert_value(header: &mut HeaderMap, value: String) -> String {
        if value.len() == 0 {
            return value;
        }
        if value.as_bytes()[0] == b'$' {
            if let Some(convert) = header.system_get(&value) {
                println!("get {} convert {}", value, convert);
                return convert.to_string();
            } else {
                return "unknown".to_string();
            }
        }
        return value;
    }

}
