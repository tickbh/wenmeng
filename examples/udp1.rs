use tokio::net::UdpSocket;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let sock = UdpSocket::bind("127.0.0.1:2222").await?;
    sock.connect("127.0.0.1:54323").await?;

    let mut buf = [0; 1024];
    loop {
        let len = 10;
        let len = sock.send(&buf[..len]).await?;
        println!("{:?} bytes sent", len);

        let len = sock.recv(&mut buf).await?;
        println!("{:?} bytes received from {:?}", len, "a");
    }
}