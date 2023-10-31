

use webparse::{Request, Buf};
use wenmeng::{Client, ProtResult};

async fn test_http2() -> ProtResult<()> {
    // let url = "http://nghttp2.org/";

    println!("aaaaaaaaaaaaaa");
    // let url = "http://localhost:82/root/target/rid_maps.log";
    let url = "http://localhost:82/root/target/";
    // let mut vecs = vec![];
    // tokio::time::sleep(Duration::from_secs(100000)).await;
    let req = Request::builder().method("GET").url(url).body("").unwrap();
    println!("url = {:?}", req.get_connect_url());
    let client = Client::builder().
        // http2(false)
        http2_only(true)
        .connect(url).await.unwrap();

    let (mut recv, sender) = client.send2(req.into_type()).await?;
    let mut res = recv.recv().await.unwrap();
    res.body_mut().wait_all().await;
    println!("res = {} {}", res.status(), res.body_mut().origin_len());

    let req = Request::builder()
        .method("GET")
        .url(url.to_owned() + "blog/")
        .body("")
        .unwrap();
    println!("url = {:?}", req.url());
    sender.send(req.into_type()).await.unwrap();
    let mut res = recv.recv().await.unwrap();
    res.body_mut().wait_all().await;
    println!("res = {} {}", res.status(), res.body_mut().origin_len());
    println!("res = {:?}", res.body_mut().binary().chunk());
    // tokio::time::sleep(Duration::from_secs(100000)).await;
    Ok(())
}

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
        // .http2_only(true)
        .connect_tls(req.url().clone())
        .await
        .unwrap();

    let mut recv = client.send(req.into_type()).await.unwrap();
    while let Some(mut res) = recv.recv().await {
        res.body_mut().wait_all().await;
        println!("res = {}", res);
    }

    Ok(())
    // println!("res = {:?}", res);
}

#[tokio::main]
async fn main() {
    test_http2().await;
    // test_https2().await;
    return;
}
