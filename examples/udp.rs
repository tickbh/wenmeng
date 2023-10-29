use tokio::net::UdpSocket;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:8089").await?;
    let mut buf = [0; 1024];
    loop {
        let (len, addr) = sock.recv_from(&mut buf).await?;
        let mut vec = "from server: ".as_bytes().to_vec();
        vec.extend(&buf[..len]);
        let _ = sock.send_to(&vec, addr).await?;
    }
}