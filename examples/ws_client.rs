use async_trait::async_trait;
use std::{time::Duration};

use tokio::{sync::mpsc::Sender};
use webparse::{ws::{OwnedMessage, CloseData}};
use wenmeng::{
    self,
    ws::{WsHandshake, WsOption, WsTrait}, Client, ProtResult,
};

// #[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

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
            .send(OwnedMessage::Text("from client".to_string()))
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
            .send(OwnedMessage::Close(Some(CloseData::normal()))).await;
        Ok(())
    }
}

async fn run_main() -> ProtResult<()> {
    // 在main函数最开头调用这个方法
    let url = "ws://127.0.0.1:8081";

    // let url = "http://localhost:8080/";

    let mut client = Client::builder()
        .http2(false)
        .url(url)
        .unwrap()
        .connect()
        .await
        .unwrap();
    client.set_callback_ws(Box::new(Operate { sender: None }));
    client.wait_ws_operate().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    if let Err(e) = run_main().await {
        println!("运行wmproxy发生错误:{:?}", e);
    }
}
