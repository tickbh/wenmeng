//! A "tiny" example of HTTP request/response handling using transports.
//!
//! This example is intended for *learning purposes* to see how various pieces
//! hook up together and how HTTP can get up and running. Note that this example
//! is written with the restriction that it *can't* use any "big" library other
//! than Tokio, if you'd like a "real world" HTTP library you likely want a
//! crate like Hyper.
//!
//! Code here is based on the `echo-threads` example and implements two paths,
//! the `/plaintext` and `/json` routes to respond with some text and json,
//! respectively. By default this will run I/O on all the cores your system has
//! available, and it doesn't support HTTP request bodies.

#![warn(rust_2018_idioms)]

use webparse::{Binary, BinaryMut, Request, Response};
#[macro_use]
extern crate serde_derive;
use std::{
    env,
    error::Error,
    io::{self, Read},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::channel,
};

use dianmeng::{self, ProtResult, RecvStream, SendControl, Server};

trait Xx {
    // async fn xx();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let server = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);

    loop {
        let (stream, _) = server.accept().await?;
        println!("recv = {:?}", stream);
        tokio::spawn(async move {
            if let Err(e) = process(stream).await {
                println!("failed to process connection; error = {}", e);
            }
            println!("aaaaaaaaaaaaaaaaaaaa");
        });
    }
}

async fn operate(mut req: Request<RecvStream>) -> ProtResult<Option<Response<RecvStream>>> {
    let mut builder = Response::builder().version(req.version().clone());
    let body = match &*req.url().path {
        "/plaintext" | "/" => {
            builder = builder.header("content-type", "text/plain");
            "Hello, World!".to_string()
        }
        "/post" => {
            let body = req.body_mut();

            let mut buf = [0u8; 10];
            if let Ok(len) = body.read(&mut buf) {
                println!("skip = {:?}", &buf[..len]);
            }
            let mut binary = BinaryMut::new();
            body.read_all(&mut binary).await.unwrap();
            println!("binary = {:?}", binary);

            builder = builder.header("content-type", "text/plain");
            format!("Hello, World! {:?}", TryInto::<String>::try_into(binary)).to_string()
        }
        "/json" => {
            builder = builder.header("content-type", "application/json");
            #[derive(Serialize)]
            struct Message {
                message: &'static str,
            }
            serde_json::to_string(&Message {
                message: "Hello, World!",
            })
            .unwrap()
        }
        _ => {
            builder = builder.status(404);
            String::new()
        }
    };

    let (sender, receiver) = channel(10);
    let recv = RecvStream::new(receiver, BinaryMut::from(body.into_bytes().to_vec()), false);
    let response = builder
        .body(recv)
        .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;

    let control = req.extensions_mut().remove::<SendControl>();
    if control.is_some() {
        let (sender, receiver) = channel(10);
        let recv = RecvStream::new(
            receiver,
            BinaryMut::from("push info".as_bytes().to_vec()),
            false,
        );

        let res = Response::builder()
            .version(req.version().clone())
            .header(":path", "/aaa")
            .header(":scheme", "http")
            .header(":method", "GET")
            .header(":authority", req.get_authority())
            .body(recv)
            .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;

        let _send = control.unwrap().send_response(res).await?;
        tokio::spawn(async move {
            for i in 1..20 {
                sender
                    .send((false, Binary::from(format!("hello{} ", i).into_bytes())))
                    .await;
            }
            println!("send!!!!! end!!!!!!");
            sender
                .send((true, Binary::from_static("world\r\n".as_bytes())))
                .await;
        });
        // Ok(Some(response))
    }
    tokio::spawn(async move {
        println!("send!!!!!");
        for i in 1..2 {
            sender
                .send((false, Binary::from(format!("hello{} ", i).into_bytes())))
                .await;
        }
        println!("send!!!!! end!!!!!!");
        // sender.send((true, Binary::from_static("world\r\n".as_bytes()))).await;
    });
    Ok(Some(response))
}

// async fn operate1(mut req: Request<String>) -> ProtResult<Option<Response<String>>> {
//     let mut response = Response::builder().version(req.version().clone());
//     let body = match &*req.url().path {
//         "/plaintext" => {
//             response = response.header("content-type", "text/plain");
//             "Hello, World!".to_string()
//         }
//         "/post" => {
//             let _body = req.body_mut();

//             response = response.header("content-type", "text/plain");
//             format!("Hello, World! {:?}", 111).to_string()
//         }
//         "/json" => {
//             response = response.header("content-type", "application/json");
//             #[derive(Serialize)]
//             struct Message {
//                 message: &'static str,
//             }
//             serde_json::to_string(&Message {
//                 message: "Hello, World!",
//             })
//             .unwrap()
//         }
//         _ => {
//             response = response.status(404);
//             String::new()
//         }
//     };
//     let response = response
//         .body(body)
//         .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
//     Ok(Some(response))
// }

async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {
    // let mut connect = StateHandshake::handshake(stream).await.unwrap();
    // let mut connect = dmeng::Builder::new().connection(stream);
    let mut server = Server::new(stream);
    let ret = server.incoming(operate).await;
    println!("end!!!!!!?????????????????? {:?}", ret);
    Ok(())
}
