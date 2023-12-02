use std::{env, error::Error, time::Duration};
use async_trait::async_trait;

use tokio::{net::TcpListener};
use webparse::{Response};
use wenmeng::{self, ProtResult, Server, RecvRequest, RecvResponse, OperateTrait, Middleware};

// #[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

struct Operate;

#[async_trait]
impl OperateTrait for Operate {
    async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
        tokio::time::sleep(Duration::new(1, 1)).await;
        let response = Response::builder()
            .version(req.version().clone())
            .body("Hello World\r\n".to_string())?;
        Ok(response.into_type())
    }
}

struct HelloMiddleware;
#[async_trait]
impl Middleware for HelloMiddleware {
    async fn process_request(&mut self, request: &mut RecvRequest) -> ProtResult<Option<RecvResponse>> {
        println!("hello request {}", request.url());
        Ok(None)
    }

    async fn process_response(&mut self, _request: &mut RecvRequest, response: &mut RecvResponse) -> ProtResult<()> {
        println!("hello response {}", response.status());
        Ok(())
    }
}

async fn run_main() -> Result<(), Box<dyn Error>> {
    // 在main函数最开头调用这个方法
    let file_name = format!("heap-{}.json", std::process::id());
    // let _profiler = dhat::Profiler::builder().file_name(file_name).build();
    //
    // let _profiler = dhat::Profiler::new_heap();


    // env_logger::init();
    // console_subscriber::init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let server = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);
    loop {
        let (stream, addr) = server.accept().await?;
        tokio::spawn(async move {
            let mut server = Server::new(stream, Some(addr));
            server.middle(HelloMiddleware);
            let operate = Operate;
            let e = server.incoming(operate).await;
            println!("close server ==== addr = {:?} e = {:?}", addr, e);
        });
    }
}


fn main() {
    env_logger::init();
    use tokio::runtime::Builder;
    let runtime = Builder::new_multi_thread()
        .enable_io()
        .worker_threads(4)
        .enable_time()
        .thread_name("wmproxy")
        .thread_stack_size(10 * 1024 * 1024)
        .build()
        .unwrap();
    runtime.block_on(async {
        if let Err(e) = run_main().await {
            println!("运行wmproxy发生错误:{:?}", e);
        }
    })
}