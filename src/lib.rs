
mod protocol;

pub use protocol::*;
use webparse::{Request, Response};

pub type RecvRequest = Request<RecvStream>;
pub type RecvResponse = Response<RecvStream>;

use async_trait::async_trait;

#[async_trait]
pub trait OperateTrait {
    async fn operate(&mut self, req: &mut RecvRequest) -> ProtResult<RecvResponse>;
}