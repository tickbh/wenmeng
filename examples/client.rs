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
// Created Date: 2023/09/12 02:36:35

use std::time::{Instant};
use webparse::{HeaderName, Request};
use wenmeng::{Client, ProtResult};

async fn test_http2() -> ProtResult<()> {
    let url = "http://nghttp2.org/";

    // println!("aaaaaaaaaaaaaa");
    // // let url = "http://localhost:82/root/target/rid_maps.log";
    // // let url = "http://127.0.0.1:8080/root/README.md";
    // let url = "http://www.baidu.com";
    // let req = Request::builder().method("GET").url(url).body("").unwrap();
    // println!("url = {:?} now = {:?}", req.get_connect_url(), Instant::now());
    // let client = Client::builder()
    //     .connect(url).await.unwrap();
    // let (mut recv, _sender) = client.send2(req.into_type()).await?;
    // let res = recv.recv().await;

    // let mut vecs = vec![];
    // tokio::time::sleep(Duration::from_secs(100000)).await;
    let req = Request::builder()
        .method("GET")
        .header(HeaderName::ACCEPT_ENCODING, "gzip")
        .url(url)
        .body("")
        .unwrap();
    println!(
        "url = {:?} now = {:?}",
        req.get_connect_url(),
        Instant::now()
    );
    
    let client = Client::builder()
        // .http2(false)
        // .http2_only(true)
        // .connect_timeout(Duration::new(1, 10))
        // .ka_timeout(Duration::new(10, 10))
        // .read_timeout(Duration::new(0, 1))
        // .add_proxy("socks5://wmproxy:wmproxy@127.0.0.1:8090")?
        .add_proxy("http://wmproxy:wmproxy@127.0.0.1:8090")?
        .url(url)?
        // .write_timeout(Duration::new(0, 1))
        .connect()
        .await
        .unwrap();

    let mut res = client.send_now(req.into_type()).await.unwrap();
    res.body_mut().wait_all().await;
    println!(
        "res = {} {} compress  = {}",
        res.status(),
        res.body_mut().origin_len(),
        res.body_mut().get_origin_compress()
    );

    let res = res.into_type::<String>();
    println!("return body = {}", unsafe {
        res.body().as_str().get_unchecked(0..10)
    });

    // // let req = Request::builder()
    // //     .method("GET")
    // //     .url(url.to_owned() + "blog/")
    // //     .body("")
    // //     .unwrap();
    // // println!("url = {:?}", req.url());
    // // sender.send(req.into_type()).await.unwrap();

    // let _res = recv.recv().await.unwrap();
    // res.body_mut().wait_all().await;
    // println!("res = {} {}", res.status(), res.body_mut().origin_len());
    // println!("res = {:?}", res.body_mut().binary().chunk());
    // tokio::time::sleep(Duration::from_secs(100000)).await;
    Ok(())
}

#[allow(dead_code)]
async fn test_https2() -> ProtResult<()> {
    // let req = Request::builder().method("GET").url("http://nghttp2.org/").upgrade_http2(settings).body("").unwrap();
    let req = Request::builder()
        .method("GET")
        .url("https://nghttp2.org/")
        // .header("accept", "*/*")
        .body("")
        .unwrap();

    // let req = Request::builder().method("GET").url("http://www.baidu.com/").upgrade_http2().body("").unwrap();
    println!("req = {}", req);
    let client = Client::builder()
        .url(req.url().clone())?
        // .http2_only(true)
        .connect()
        .await
        .unwrap();

    let mut recv = client.send(req.into_type()).await.unwrap();
    while let Some(res) = recv.recv().await {
        let mut res = res?;
        res.body_mut().wait_all().await;
        println!("res = {}", res);
    }

    Ok(())
    // println!("res = {:?}", res);
}

#[tokio::main]
async fn main() {
    env_logger::init();
    for _ in 0..10 {
        if let Err(e) = test_http2().await {
            println!("发生错误:{:?}", e);
        }
    }
    // test_https2().await;
    return;
}
