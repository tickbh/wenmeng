use std::{env, error::Error};
use tokio::{net::TcpListener};
use webparse::{Request, Response};
use wenmeng::{self, ProtResult, RecvStream, Server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let server = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);
    loop {
        let (stream, addr) = server.accept().await?;
        tokio::spawn(async move {
            let mut server = Server::new(stream, Some(addr));
            async fn operate(req: Request<RecvStream>) -> ProtResult<Response<String>> {
                let response = Response::builder()
                    .version(req.version().clone())
                    .body("Hello World\r\n".to_string())?;
                Ok(response)
            }
            let _ = server.incoming(operate).await;
        });
    }
}
