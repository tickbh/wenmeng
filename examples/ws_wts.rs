use async_trait::async_trait;
use std::{io, time::Duration};

use tokio::{
    net::{tcp, TcpListener},
    sync::mpsc::{channel, Receiver, Sender},
};
use webparse::ws::{CloseData, OwnedMessage};
use wenmeng::{
    self,
    plugins::{StreamToWs, WsToStream},
    ws::{WsHandshake, WsOption, WsTrait},
    Client, ProtResult,
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
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    if let Err(e) = run_main().await {
        println!("运行wmproxy发生错误:{:?}", e);
    }
}
