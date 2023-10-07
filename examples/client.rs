use webparse::{
    http2::frame::{Frame, Settings},
    Request,
};
use wenmeng::{Client, RecvStream, ProtResult};

async fn test_http2() -> ProtResult<()> {
    let url = "http://127.0.0.1:8080/"; //"http://nghttp2.org/"
    let mut settings = Settings::default();
    settings.set_enable_push(false);
    let req = Request::builder()
        .method("GET")
        .url(url)
        .body("")
        .unwrap();

    let client = Client::builder()
        .http2(false)
        .request(&req)
        .await
        .unwrap();

    let (mut recv, sender) = client.send2(req.into_type()).await?;
    let mut res = recv.recv().await.unwrap();
    res.body_mut().wait_all().await;
    println!("res = {}", res);

    let req = Request::builder()
        .method("GET")
        .url(url.to_owned() + "blog")
        .body("")
        .unwrap();
    sender.send(req.into_type()).await?;
    let res = recv.recv().await.unwrap();
    println!("res = {}", res);

    Ok(())
    // println!("res = {:?}", res);
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
    let mut client = Client::builder()
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
    let req = Request::builder()
        .method("GET")
        .url("http://www.baidu.com")
        .body(())
        .unwrap();
    let client = Client::builder().request(&req).await.unwrap();
    let mut res = client.send(req.into_type()).await.unwrap().recv().await;
    println!("res = {:?}", res);
    res.as_mut().unwrap().body_mut().wait_all().await;
    println!("res = {}", res.unwrap());

    //
    // let client = reqwest::Client::builder().http2_prior_knowledge().build().unwrap();
    // // let x = client.request(reqwest::Method::GET, "http://192.168.179.133:8080/post").send().await.unwrap();
    // // println!("x = {:?}", x);
    // let x = client.request(reqwest::Method::GET, "http://nghttp2.org/post").send().await.unwrap();
    // println!("x = {:?}", x);
    // let body = reqwest::get("https://www.rust-lang.org")
    // .await.unwrap()
    // .text()
    // .await.unwrap();

    // println!("body = {:?}", body);
}
