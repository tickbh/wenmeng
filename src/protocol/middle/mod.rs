use async_trait::async_trait;

use crate::{RecvRequest, ProtResult, RecvResponse};


#[async_trait]
pub trait Middleware: Send + Sync {
    async fn process_request(&mut self, request: &mut RecvRequest) -> ProtResult<()>;
    async fn process_response(&mut self, request: &mut RecvRequest, response: &mut RecvResponse) -> ProtResult<()>;
}