use webparse::{Request, http2::frame::{Frame, Settings}};
use wenmeng::{Client, RecvStream};


async fn test_http2() {
    let mut settings = Settings::default();
    settings.set_enable_push(false);
    let req = Request::builder().method("GET").url("http://nghttp2.org/").upgrade_http2(settings).body("").unwrap();
    // let req = Request::builder().method("GET").url("http://www.baidu.com/").upgrade_http2().body("").unwrap();
    println!("req = {}", req);
    let client = Client::builder().request(&req).await.unwrap();
    let mut res = client.send(req.into_type()).await.unwrap().recv().await;
    // println!("res = {:?}", res);
    res.as_mut().unwrap().body_mut().wait_all().await;
    println!("res = {}", res.unwrap());
}

#[tokio::main]
async fn main() {

    test_http2().await;
    return;
    let req = Request::builder().method("GET").url("http://www.baidu.com").body(()).unwrap();
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