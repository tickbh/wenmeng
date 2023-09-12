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

use bytes::BytesMut;
use futures::SinkExt;
use webparse::{Request, Response, http::{StatusCode, http2::frame::Frame}, Binary, BinaryMut};
#[macro_use]
extern crate serde_derive;
use std::{env, error::Error, fmt::{self,}, io::{self, Read}, borrow::BorrowMut, time::Duration};
use tokio::{net::{TcpListener, TcpStream}, sync::mpsc::{channel, Sender}};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};

use dmeng::{self, H2Connection, StateHandshake, SendControl, Server, RecvStream, ProtoResult, SendStream};

trait Xx {
    // async fn xx();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    // use std::sync::mpsc::channel;

    // let (tx, rx) = channel();

    // // This send is always successful
    // tx.send(1).unwrap();
    // tx.send(2).unwrap();
    // tx.send(3).unwrap();
    // for i in 0..1000 {
    //     tx.send(i).unwrap();
    // }


    // for i in 0..1000 {
    //     rx.recv().unwrap();
    // }
    // // This send will fail because the receiver is gone
    // // drop(rx);
    // // assert_eq!(tx.send(1).unwrap_err().0, 1);

    // println!("aaa {:?}", rx.recv());
    // println!("aaa {:?}", rx.recv());
    // println!("aaa {:?}", rx.recv());
    // println!("aaa {:?}", rx.recv());

    // Parse the arguments, bind the TCP socket we'll be listening to, spin up
    // our worker threads, and start shipping sockets to those worker threads.
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

async fn operate(mut req: Request<RecvStream>) -> ProtoResult<Option<Response<RecvStream>>> {
    let mut response = Response::builder().version(req.version().clone());
    let body = match &*req.url().path {
        "/plaintext" => {
            response = response.header("content-type", "text/plain");
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

            response = response.header("content-type", "text/plain");
            format!("Hello, World! {:?}", TryInto::<String>::try_into(binary)).to_string()
        }
        "/json" => {
            response = response.header("content-type", "application/json");
            #[derive(Serialize)]
            struct Message {
                message: &'static str,
            }
            serde_json::to_string(&Message {
                message: "Hello, World!",
            }).unwrap()
        }
        _ => {
            response = response.status(404);
            String::new()
        }
    };
    let (sender, receiver) = channel(10);
    let recv = RecvStream::new(receiver, BinaryMut::from(body.into_bytes().to_vec()), false);
    let response = response
        .body( recv )
        .map_err(|err| io::Error::new(io::ErrorKind::Other, ""))?;

    let control = req.extensions_mut().get_mut::<SendControl>();
    if control.is_some() {
        let mut send = control.unwrap().send_response(response, false).unwrap();
        tokio::spawn(async move {
            for i in 1..8 {
                send.send_data(Binary::from(format!("hello{} ", i).into_bytes()), false);
                tokio::time::sleep(Duration::new(0, 1000)).await;
            }
            send.send_data(Binary::from_static("world\r\n".as_bytes()), true);
        });
        Ok(None)
    } else {
        tokio::spawn(async move {
            println!("send!!!!!");
            for i in 1..2 {
                sender.send((false, Binary::from(format!("hello{} ", i).into_bytes()))).await;
            }
            println!("send!!!!! end!!!!!!");
            sender.send((true, Binary::from_static("world\r\n".as_bytes()))).await;

        });
        Ok(Some(response))
    }
    
}


async fn operate1(mut req: Request<String>) -> ProtoResult<Option<Response<String>>> {
    let mut response = Response::builder().version(req.version().clone());
    let body = match &*req.url().path {
        "/plaintext" => {
            response = response.header("content-type", "text/plain");
            "Hello, World!".to_string()
        }
        "/post" => {
            let body = req.body_mut();

            response = response.header("content-type", "text/plain");
            format!("Hello, World! {:?}", 111).to_string()
        }
        "/json" => {
            response = response.header("content-type", "application/json");
            #[derive(Serialize)]
            struct Message {
                message: &'static str,
            }
            serde_json::to_string(&Message {
                message: "Hello, World!",
            }).unwrap()
        }
        _ => {
            response = response.status(404);
            String::new()
        }
    };
    let response = response
        .body( body )
        .map_err(|err| io::Error::new(io::ErrorKind::Other, ""))?;
    Ok(Some(response))
}

async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {

    // let mut connect = StateHandshake::handshake(stream).await.unwrap();
    // let mut connect = dmeng::Builder::new().connection(stream);
    let mut server = Server::new(stream);
    let ret = server.incoming(operate1).await;
    // while let Ok(Some(_)) = ret {

    // }
    println!("end!!!!!!?????????????????? {:?}", ret);
    // while let Some(request) = server.incoming(operate).await {
    //     match request {
    //         Ok(request) => {
    //             println!("request === {:?}", request);
    //             respond(request).await;

    //             // println!("response === {:?}", response);
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             // panic!("aaaaaa");
    //         },
    //     }
    // }
    // let mut transport = Framed::new(stream, Http(None));

    // while let Some(request) = transport.next().await {
    //     match request {
    //         Ok(request) => {
    //             let response = respond(request).await?;
    //             transport.send(response).await?;
    //         }
    //         Err(e) => return Err(e.into()),
    //     }
    // }

    Ok(())
}



