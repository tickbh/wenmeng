

// #[tokio::main]
// async fn main() -> std::io::Result<()> {
//     use std::{future::poll_fn, task::Poll, pin::Pin};
//     use tokio::net::TcpListener;
//     let mut listeners = vec![];
//     // 同时监听若干个端口
//     for i in 1024..1099 {
//         listeners.push(TcpListener::bind(format!("127.0.0.1:{}", i)).await?);
//     }
//     loop {
//         let mut pin_listener = Pin::new(&mut listeners);
//         // 同时监听若干个端口，只要有任一个返回则返回数据
//         let fun = poll_fn(|cx| {
//             for l in &*pin_listener.as_mut() {
//                 match l.poll_accept(cx) {
//                     v @ Poll::Ready(_) => return v,
//                     Poll::Pending => {},
//                 }
//             }
//             Poll::Pending
//         });
//         let (conn, addr) = fun.await?;
//         println!("receiver conn:{:?} addr:{:?}", conn, addr);
//     }
// }

use tokio::{net::TcpListener, io::{AsyncReadExt, AsyncWriteExt}};


#[tokio::main]
async fn main() -> std::io::Result<()> {
    let tcp_listener = TcpListener::bind(format!("127.0.0.1:{}", 8082)).await?;
    loop {
        let mut stream = tcp_listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = vec![0;20480];
            loop {
                if let Ok(size) = stream.0.read(&mut buf).await {
                    println!("receiver = {:?} size = {:?}", &buf[..size], size);
                    let _ = stream.0.write_all(b"from tcp:").await;
                    let _ = stream.0.write_all(&buf[..size]).await;
                } else {
                    break;
                }
            }
        });
    }
}