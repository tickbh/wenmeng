


use tokio::{
    net::{TcpListener},
};

use wenmeng::{
    self,
    plugins::{WsToStream}, ProtResult,
};

async fn run_main() -> ProtResult<()> {
    let tcp_listener = TcpListener::bind("127.0.0.1:8081").await?;
    loop {
        let stream = tcp_listener.accept().await?;
        let ws_to_stream = WsToStream::new(stream.0, "127.0.0.1:8082")?;
        tokio::spawn(async move {
            let _ = ws_to_stream.copy_bidirectional().await;
        });
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    if let Err(e) = run_main().await {
        println!("运行wmproxy发生错误:{:?}", e);
    }
}
