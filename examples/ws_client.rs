use async_trait::async_trait;
use std::{time::Duration, io};

use tokio::{sync::mpsc::{Sender, Receiver, channel}};
use webparse::{ws::{OwnedMessage, CloseData}};
use wenmeng::{
    self,
    ws::{WsHandshake, WsOption, WsTrait}, Client, ProtResult,
};

struct Operate {
    sender: Option<Sender<OwnedMessage>>,
    receiver: Option<Receiver<OwnedMessage>>,
}

#[async_trait]
impl WsTrait for Operate {
    async fn on_open(&mut self, shake: WsHandshake) -> ProtResult<Option<WsOption>> {
        // 将receiver传给控制中心, 以让其用该receiver做接收
        let mut option = WsOption::new();
        option.set_interval(Duration::from_secs(1000));
        if self.receiver.is_some() {
            option.set_receiver(self.receiver.take().unwrap());
        }
        if self.sender.is_none() {
            self.sender = Some(shake.sender);
        }
        Ok(Some(option))
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
    // 自己手动构建数据对,并将receiver传给服务端
    let (sender, receiver) = channel(10);
    let sender_clone = sender.clone();
    tokio::spawn(async move {
        let url = "ws://127.0.0.1:8081";
        let mut client = Client::builder()
            .url(url)
            .unwrap()
            .connect()
            .await
            .unwrap();
        client.set_callback_ws(Box::new(Operate { sender:Some(sender_clone), receiver: Some(receiver) }));
        client.wait_ws_operate().await.unwrap();
    });
    loop {
        let mut buffer = String::new();
        let stdin = io::stdin(); // We get `Stdin` here.
        stdin.read_line(&mut buffer)?;
        sender.send(OwnedMessage::Text(buffer)).await?;
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
