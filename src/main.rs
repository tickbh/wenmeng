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
use webparse::{Request, Response, http::{StatusCode, http2::frame::Frame}, Binary};
#[macro_use]
extern crate serde_derive;
use std::{env, error::Error, fmt::{self,}, io, borrow::BorrowMut, time::Duration};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};

use dmeng::{self, Http, Connection, StateHandshake, SendControl};

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
        tokio::spawn(async move {
            if let Err(e) = process(stream).await {
                println!("failed to process connection; error = {}", e);
            }
        });
    }
}

async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {

    // let mut connect = StateHandshake::handshake(stream).await.unwrap();
    let mut connect = dmeng::Builder::new().connection(stream);
    while let Some(request) = connect.incoming().await {
        match request {
            Ok(request) => {
                println!("request === {:?}", request);
                respond(request.0, request.1).await;

                // println!("response === {:?}", response);
            }
            Err(e) => {
                println!("error = {:?}", e);
                // panic!("aaaaaa");
            },
        }
    }
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

async fn respond(mut req: Request<dmeng::RecvStream>, mut control: SendControl) -> Result<(), Box<dyn Error>> {
    let mut response = Response::builder().version(req.version().clone());
    let body = match &*req.url().path {
        "/plaintext" => {
            response = response.header("content-type", "text/plain");
            "Hello, World!".to_string()
        }
        "/post" => {
            let body = req.body_mut();
            let binary = body.read_all().await.unwrap();

            // body.
            response = response.header("content-type", "text/plain");
            format!("Hello, World! {:?}", TryInto::<String>::try_into(binary)).to_string()
            // "".to_string()
        }
        "/json" => {
            response = response.header("content-type", "application/json");
            #[derive(Serialize)]
            struct Message {
                message: &'static str,
            }
            serde_json::to_string(&Message {
                message: "Hello, World!",
            })?
        }
        _ => {
            response = response.status(404);
            String::new()
        }
    };
    let response = response
        .body( Binary::from(body.into_bytes()))
        .map_err(|err| io::Error::new(io::ErrorKind::Other, ""))?;

    let mut send = control.send_response(response, false).unwrap();
    tokio::spawn(async move {
        for i in 1..99 {
            send.send_data(Binary::from(format!("hello{} ", i).into_bytes()), false);
            tokio::time::sleep(Duration::new(0, 1000)).await;
        }
        send.send_data(Binary::from_static("world\r\n".as_bytes()), true);
    });
    
    Ok(())
}


