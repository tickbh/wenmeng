
mod protocol;

pub use protocol::*;
use webparse::{Request, Response};

pub type RecvRequest = Request<RecvStream>;
pub type RecvResponse = Response<RecvStream>;

use async_trait::async_trait;

#[async_trait]
pub trait OperateTrait {
    async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse>;
    
    async fn middle_operate(&mut self, req: &mut RecvRequest, middles: &mut Vec<Box<dyn Middleware>>) -> ProtResult<()> {
        let _req = req;
        let _middle = middles;
        Ok(())
    }
}