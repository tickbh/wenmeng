use super::plugin_trait::PluginTrait;
use crate::ProtResult;
use std::{future::Future, path::Path, io};
use tokio::fs::File;
use webparse::{Response, Request, BinaryMut, HeaderName};
use crate::RecvStream;

pub struct FileServer;

impl FileServer {
    pub async fn deal_request(req: Request<RecvStream>, mut root: String, prefix: String) -> ProtResult<Option<Response<RecvStream>>> {
        let path = req.path().clone();
        if !path.starts_with(&prefix) {
            return Ok(Some(Response::builder().status(404).body("unknow path").unwrap().into_type())) 
        }
        if root.is_empty() {
            root = std::env::current_dir()?.to_str().unwrap().to_string();
        }
        let root_path = Path::new(&root);
        let href = "/".to_string() + path.strip_prefix(&prefix).unwrap();
        let real_path = root.clone() + &href;
        let real_path = Path::new(&real_path);
        if !real_path.starts_with(root_path) {
            return Ok(Some(Response::builder().status(404).body("can't view parent file").unwrap().into_type())) 
        }

        if real_path.is_dir() {
            let mut binary = BinaryMut::new();
            binary.put_slice("<table>\r\n<tbody>".as_bytes());

            for entry in real_path.read_dir()? {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let new = path.strip_prefix(root_path).unwrap();
                    binary.put_slice("<tr>".as_bytes());
                    let href = new.to_str().unwrap();
                    let filename = path.file_name().unwrap().to_str().unwrap();
                    if path.is_dir() {
                        binary.put_slice("<td>folder</td>".as_bytes());
                        binary.put_slice(format!("<td><a href=\"{}{}\">{}</td>", prefix, href, filename).as_bytes());
                    } else {
                        binary.put_slice("<td>file</td>".as_bytes());
                        binary.put_slice(format!("<td><a href=\"{}{}\">{}</td>", prefix, href, filename).as_bytes());
                    }
                    binary.put_slice("</tr>".as_bytes());
                    println!("{:?}", entry.path());
                }
            }
            binary.put_slice("</td>\r\n</a>".as_bytes());
            let mut recv = RecvStream::only(binary.freeze());
            let mut builder = Response::builder().version(req.version().clone());
            let response = builder
                // .header("Content-Length", length as usize)
                .header(HeaderName::CONTENT_ENCODING, "gzip")
                .header(HeaderName::TRANSFER_ENCODING, "chunked")
                .body(recv)
                .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
            return Ok(Some(response))

        } else {
            let file = File::open(real_path).await?;
            let length = file.metadata().await?.len();
            // let recv = RecvStream::new_file(file, BinaryMut::from(body.into_bytes().to_vec()), false);
            let mut recv = RecvStream::new_file(file, BinaryMut::new(), false);
            // recv.set_compress_origin_gzip();
            let mut builder = Response::builder().version(req.version().clone());
            let response = builder
                // .header("Content-Length", length as usize)
                .header(HeaderName::CONTENT_ENCODING, "gzip")
                .header(HeaderName::TRANSFER_ENCODING, "chunked")
                .body(recv)
                .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
            return Ok(Some(response))
        }
        

        Ok(None)
    }

}

// impl PluginTrait for FileServer {
//     type NextFuture = dyn Future<Output = ProtResult<Option<Response<RecvStream>>>>;

//     fn deal_request(req: webparse::Request<crate::RecvStream>) -> Self::NextFuture {
//         todo!()
//     }
// }