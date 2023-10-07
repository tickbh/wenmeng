# dianmeng

一个包含http1.1及http2的服务器实现, 依赖tokio实现

## 使用方法

简单的hello world示例

```rust
use std::{env, error::Error};
use tokio::net::TcpListener;
use webparse::{Request, Response};
use dianmeng::{self, ProtResult, RecvStream, Server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let server = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);
    loop {
        let (stream, _) = server.accept().await?;
        tokio::spawn(async move {
            let mut server = Server::new(stream);
            async fn operate(req: Request<RecvStream>) -> ProtResult<Option<Response<String>>> {
                let response = Response::builder()
                    .version(req.version().clone())
                    .body("Hello World".to_string())?;
                Ok(Some(response))
            }
            let _ = server.incoming(operate).await;
        });
    }
}
```

## 客户端使用方法

> http1/http2通用, recv可以接收多个返回及服务端的推送信息
```rust
use webparse::Request;
use wenmeng::{Client, ProtResult};

async fn test_http2() -> ProtResult<()> {
    let url = "http://nghttp2.org/"; //"http://127.0.0.1:8080/"
    let req = Request::builder().method("GET").url(url).body("").unwrap();

    let client = Client::builder().connect(url).await.unwrap();

    let (mut recv, sender) = client.send2(req.into_type()).await?;
    let mut res = recv.recv().await.unwrap();
    res.body_mut().wait_all().await;
    println!("res = {}", res);

    let req = Request::builder()
        .method("GET")
        .url(url.to_owned() + "blog/")
        .body("")
        .unwrap();
    sender.send(req.into_type()).await?;
    let res = recv.recv().await.unwrap();
    println!("res = {}", res);
    Ok(())
}
```

## License
Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE) or [https://apache.org/licenses/LICENSE-2.0](https://apache.org/licenses/LICENSE-2.0))