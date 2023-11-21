use crate::{Middleware, HeaderHelper};

use async_trait::async_trait;

use crate::{ProtError, ProtResult, RecvRequest, RecvResponse};

pub struct BaseMiddleware {
    is_client: bool,
}

impl BaseMiddleware {
    pub fn new(is_client: bool) -> Self {
        Self { is_client }
    }
}

#[async_trait]
impl Middleware for BaseMiddleware {
    async fn process_request(&mut self, request: &mut RecvRequest) -> ProtResult<()> {
        HeaderHelper::process_request_header(request.version(), self.is_client, request)?;
        Ok(())
    }
    async fn process_response(
        &mut self,
        _request: &mut RecvRequest,
        response: &mut RecvResponse,
    ) -> ProtResult<()> {
        HeaderHelper::process_response_header(response.version(), self.is_client, response)?;
        Ok(())
    }
}
