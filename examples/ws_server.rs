use async_trait::async_trait;
use std::{error::Error, time::Duration};

use tokio::{net::TcpListener, sync::mpsc::Sender};
use webparse::ws::{CloseData, OwnedMessage};
use wenmeng::{
    self,
    ws::{WsHandshake, WsOption, WsTrait},
    ProtResult, Server,
};

struct Operate {
    sender: Option<Sender<OwnedMessage>>,
}

#[async_trait]
impl WsTrait for Operate {
    fn on_open(&mut self, shake: WsHandshake) -> ProtResult<Option<WsOption>> {
        self.sender = Some(shake.sender);
        Ok(Some(WsOption::new(Duration::from_secs(10))))
    }

    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()> {
        println!("callback on message = {:?}", msg);
        let _ = self
            .sender
            .as_mut()
            .unwrap()
            .send(OwnedMessage::Text("from server".to_string()))
            .await;
        let _ = self.sender.as_mut().unwrap().send(msg).await;
        Ok(())
    }

    async fn on_interval(&mut self, _option: &mut Option<WsOption>) -> ProtResult<()> {
        println!("on_interval!!!!!!!");
        let _ = self
            .sender
            .as_mut()
            .unwrap()
            .send(OwnedMessage::Close(Some(CloseData::normal())))
            .await;
        Ok(())
    }
}

async fn run_main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:8081".to_string();
    let server = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);
    loop {
        let (stream, addr) = server.accept().await?;
        tokio::spawn(async move {
            let mut server = Server::new(stream, Some(addr));
            let operate = Operate { sender: None };
            // 设置服务回调
            server.set_callback_ws(Box::new(operate));
            let e = server.incoming().await;
            println!("close server ==== addr = {:?} e = {:?}", addr, e);
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
