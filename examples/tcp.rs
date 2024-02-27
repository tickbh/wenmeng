

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