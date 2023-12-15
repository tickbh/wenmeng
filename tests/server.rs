// #![deny(warnings)]
#![deny(rust_2018_idioms)]

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde::Serialize;
    use std::{
        env,
        error::Error,
        io::{self, Read},
        net::SocketAddr,
    };
    use tokio::{
        fs::File,
        io::AsyncReadExt,
        net::{TcpListener, TcpStream},
    };
    use webparse::{BinaryMut, Buf, HeaderName, Request, Response, Version, Method};

    use wenmeng::{
        self, Body, Client, OperateTrait, ProtResult, RecvRequest, RecvResponse, Server,
    };

    struct Operate;

    #[async_trait]
    impl OperateTrait for Operate {
        async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
            let mut builder = Response::builder().version(req.version().clone());
            let body = match &*req.url().path {
                "/plaintext" | "/" => {
                    builder = builder.header("content-type", "text/plain; charset=utf-8");
                    Body::new_text("Hello, World!".to_string())
                }
                "/post" => {
                    if let Some(value) = req.headers().get_option_value(&HeaderName::CONTENT_LENGTH) {
                        println!("value = {:?}", value.as_string());
                    }
                    let body = req.body_mut();
                    let mut binary = BinaryMut::new();
                    body.read_all(&mut binary).await.unwrap();
                    println!("binary = {:?}", binary.remaining());
                    builder = builder.header("content-type", "text/plain");
                    Body::new_binary(binary)
                    // format!("Hello, World! {:?}", TryInto::<String>::try_into(binary)).to_string()
                }
                "/json" => {
                    builder = builder.header("content-type", "application/json");
                    #[derive(Serialize)]
                    struct Message {
                        message: &'static str,
                    }
                    Body::new_text(
                        serde_json::to_string(&Message {
                            message: "Hello, World!",
                        })
                        .unwrap(),
                    )
                }
                "/file" => {
                    builder = builder.header("content-type", "application/json");
                    let file = File::open("README.md").await?;
                    let length = file.metadata().await?.len();
                    Body::new_file(file, length)
                }
                _ => {
                    builder = builder.status(404);
                    Body::empty()
                }
            };

            // if body.is_end() {
            //     builder = builder.header("Content-Length", body.origin_len());
            //     println!("length = {}", body.origin_len());
            // } else {
            //     builder = builder.header(HeaderName::TRANSFER_ENCODING, "chunked");
            // }

            let response = builder
                // .header("Content-Length", length as usize)
                // .header(HeaderName::TRANSFER_ENCODING, "chunked")
                .body(body)
                .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
            Ok(response)
        }
    }

    async fn process(stream: TcpStream, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let mut server = Server::new(stream, Some(addr));
        let _ret = server.incoming(Operate).await;
        Ok(())
    }

    async fn run_server() -> ProtResult<SocketAddr> {
        env_logger::init();
        let addr = "127.0.0.1:0".to_string();
        let server = TcpListener::bind(&addr).await?;
        let addr = server.local_addr()?;
        println!("Listening on: {}", addr);
        tokio::spawn(async move {
            loop {
                if let Ok((stream, addr)) = server.accept().await {
                    tokio::spawn(async move {
                        if let Err(e) = process(stream, addr).await {
                            println!("failed to process connection; error = {}", e);
                        }
                    });
                }
            }
        });
        Ok(addr)
    }

    async fn run_client(addr: SocketAddr, path: &str, must_be: BinaryMut, method: &str, body: Vec<u8>) -> ProtResult<()> {
        let url = &*format!("http://{}{}", addr, path);
        fn do_build_req(url: &str, method: &str, body: &Vec<u8>) -> Request<Body> {
            let body = BinaryMut::from(body.clone());
            Request::builder()
                .method(method)
                .url(&*url)
                .body(Body::new_binary(body))
                .unwrap()
        }
        // http2 only
        {
            let client = Client::builder()
                .http2_only(true)
                .connect(&*url)
                .await
                .unwrap();

            let mut res = client.send_now(do_build_req(url, method, &body)).await?;
            let mut result = BinaryMut::new();
            res.body_mut().read_all(&mut result).await;

            assert_eq!(result.remaining(), must_be.remaining());
            assert_eq!(result.as_slice(), must_be.as_slice());
            assert_eq!(res.version(), Version::Http2);
        }
        // http1 only
        {
            let client = Client::builder().http2(false).connect(&*url).await.unwrap();

            let mut res = client.send_now(do_build_req(url, method, &body)).await?;
            let mut result = BinaryMut::new();
            res.body_mut().read_all(&mut result).await;

            assert_eq!(result.remaining(), must_be.remaining());
            assert_eq!(result.as_slice(), must_be.as_slice());
            assert_eq!(res.version(), Version::Http11);
        }
        // http1 upgrade
        {
            let client = Client::builder().connect(&*url).await.unwrap();
            let mut res = client.send_now(do_build_req(url, method, &body)).await?;
            let mut result = BinaryMut::new();
            res.body_mut().read_all(&mut result).await;

            assert_eq!(result.remaining(), must_be.remaining());
            assert_eq!(result.as_slice(), must_be.as_slice());
            assert_eq!(res.version(), Version::Http2);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_result() -> ProtResult<()> {
        let addr = run_server().await.unwrap();
        run_client(addr, "/", BinaryMut::from("Hello, World!".to_string()), "GET", vec![]).await.unwrap();
        run_client(addr, "/plaintext", BinaryMut::from("Hello, World!".to_string()), "GET", vec![]).await.unwrap();
        let value = vec![1, 2, 3, 4, 5, 6, 7];
        run_client(addr, "/post", BinaryMut::from(value.clone()), "POST", value.clone()).await.unwrap();

        Ok(())
    }
}
