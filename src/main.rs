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
use std::{env, error::Error, fmt::{self,}, io, borrow::BorrowMut};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};

use dmeng::{self, Http, Connection, StateHandshake};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

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
    while let Some(request) = connect.next().await {
        match request {
            Ok(request) => {
                println!("request === {:?}", request);
                let response = respond(request).await;
                println!("response === {:?}", response);
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

async fn respond(mut req: Request<Binary>) -> Result<Response<String>, Box<dyn Error>> {
    let mut response = Response::builder().version(req.version().clone());
    if req.is_http2() {
        if let Some(vec) = req.extensions().borrow_mut().remove::<Vec<Frame<Binary>>>() {
            response.extensions_ref().unwrap().borrow_mut().insert(vec);
        }
    }
    
    let body = match &*req.url().path {
        "/plaintext" => {
            response = response.header("Content-Type", "text/plain");
            "Hello, World!".to_string()
        }
        "/json" => {
            response = response.header("Content-Type", "application/json");

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
        .body(body)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, ""))?;

    Ok(response)
}


