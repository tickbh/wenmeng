use std::{env, error::Error, time::Duration};
use async_trait::async_trait;
use serde::ser;
use tokio::{net::TcpListener};
use webparse::{Request, Response};
use wenmeng::{self, ProtResult, RecvStream, Server, RecvRequest, RecvResponse, OperateTrait};

struct Operate;

#[async_trait]
impl OperateTrait for Operate {
    async fn operate(&self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
        tokio::time::sleep(Duration::new(1, 1)).await;
        let response = Response::builder()
            .version(req.version().clone())
            .body("Hello World\r\n".to_string())?;
        Ok(response.into_type())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    
    env_logger::init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let server = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);
    loop {
        let (stream, addr) = server.accept().await?;
        tokio::spawn(async move {
            let mut server = Server::new(stream, Some(addr));
            let operate = Operate;
            let e = server.incoming(operate).await;
            println!("close server ==== addr = {:?} e = {:?}", addr, e);
        });
    }
}
