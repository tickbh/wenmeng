use async_trait::async_trait;
use std::{time::Duration, io};

use tokio::{sync::mpsc::{Sender, Receiver, channel}, net::{TcpListener, tcp}};
use webparse::{ws::{OwnedMessage, CloseData}};
use wenmeng::{
    self,
    ws::{WsHandshake, WsOption, WsTrait}, Client, ProtResult, plugins::StreamToWs,
};

async fn run_main() -> ProtResult<()> {
    let tcp_listener = TcpListener::bind("127.0.0.1:8082").await?;
    loop {
        let stream = tcp_listener.accept().await?;
        let stream_to_ws = StreamToWs::new(stream.0, "ws://127.0.0.1:8081")?;
        tokio::spawn(async move {
            let _ = stream_to_ws.copy_bidirectional().await;
        });
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    if let Err(e) = run_main().await {
        println!("运行wmproxy发生错误:{:?}", e);
    }
}
