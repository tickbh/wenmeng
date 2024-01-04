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
// Created Date: 2023/12/15 10:57:42

// #![deny(warnings)]
#![deny(rust_2018_idioms)]

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde::Serialize;
    use std::{
        error::Error,
        io::{self},
        net::SocketAddr,
    };
    use tokio::{
        fs::File,
        net::{TcpListener, TcpStream},
    };
    use webparse::{BinaryMut, Buf, Request, Response, Version};

    use wenmeng::{
        self, Body, Client, HttpTrait, ProtResult, RecvRequest, RecvResponse, Server,
    };

    struct Operate;

    #[async_trait]
    impl HttpTrait for Operate {
        async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
            let mut builder = Response::builder().version(req.version().clone());
            let body = match &*req.url().path {
                "/plaintext" | "/" => {
                    builder = builder.header("content-type", "text/plain; charset=utf-8");
                    Body::new_text("Hello, World!".to_string())
                }
                "/post" => {
                    let body = req.body_mut();
                    let mut binary = BinaryMut::new();
                    body.read_all(&mut binary).await.unwrap();
                    // println!("binary = {:?}", binary.remaining());
                    builder = builder.header("content-type", "text/plain");
                    Body::new_binary(binary)
                }
                "/json" => {
                    builder = builder.header("content-type", "application/json");
                    #[derive(Serialize)]
                    struct Message {
                        message: &'static str,
                    }
                    Body::from(
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
            let response = builder
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
        // let addr = "127.0.0.1:0".to_string();
        let addr = "127.0.0.1:9999".to_string();
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
