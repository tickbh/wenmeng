use std::{env, error::Error};
use tokio::net::TcpListener;
use webparse::{Request, Response};
use wenmeng::{self, ProtResult, RecvStream, Server};

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
            let _ = server.incoming(move |req| operate(req)).await;
        });
    }
}
