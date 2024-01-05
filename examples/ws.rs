use std::{env, error::Error, time::Duration};
use async_trait::async_trait;

use tokio::{net::TcpListener};
use webparse::{Response, OwnedMessage};
use wenmeng::{self, ProtResult, Server, RecvRequest, RecvResponse, HttpTrait, Middleware, Body, ws::WsTrait};

// #[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

struct Operate;

#[async_trait]
impl WsTrait for Operate {
    
    async fn on_message(&mut self, msg: OwnedMessage) -> ProtResult<()> {
        println!("callback on message = {:?}", msg);
        Ok(())
    }
    // async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
    //     tokio::time::sleep(Duration::new(1, 1)).await;
    //     let response = Response::builder()
    //         .version(req.version().clone())
    //         .body("Hello World\r\n".to_string())?;
    //     Ok(response.into_type())
    // }
}

async fn run_main() -> Result<(), Box<dyn Error>> {
    // 在main函数最开头调用这个方法
    let _file_name = format!("heap-{}.json", std::process::id());
    // let _profiler = dhat::Profiler::builder().file_name(file_name).build();
    //
    // let _profiler = dhat::Profiler::new_heap();

    // env_logger::init();
    // console_subscriber::init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8081".to_string());
    let server = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);
    loop {
        let (stream, addr) = server.accept().await?;
        tokio::spawn(async move {
            let recv = Body::empty();
            println!("recv = {:?}", std::mem::size_of_val(&recv));
            recv.print_debug();
            let x = vec![0;1900];
            // println!("size = {:?}", s);
            println!("size = {:?}", std::mem::size_of_val(&x));
            let mut server = Server::new(stream, Some(addr));
            println!("server size size = {:?}", std::mem::size_of_val(&server));
            // println!("size = {:?}", data_size(&server));
            let operate = Operate;
            let e = server.incoming_ws(operate).await;
            println!("close server ==== addr = {:?} e = {:?}", addr, e);
        });
    }
}


#[tokio::main]
async fn main() {
    env_logger::init();
    if let Err(e) = run_main().await {
        println!("运行wmproxy发生错误:{:?}", e);
    }
}