use std::{fmt::{Display, Pointer}, io};

use tokio::sync::mpsc::error::SendError;
use webparse::{WebError, Binary, http::http2::frame::Reason, Request, http2::frame::Settings};

use crate::RecvStream;

pub type ProtResult<T> = Result<T, ProtError>;

#[derive(Debug)]
pub enum ProtError {
    /// 标准错误库的错误类型
    IoError(io::Error),
    /// 解析库发生错误
    WebError(WebError),
    /// 其它错误信息
    Extension(&'static str),

    SendError,
    /// 协议数据升级, 第一参数表示将要写给客户端的消息, 第二参数表示原来未处理的请求
    ServerUpgradeHttp2(Binary, Option<Request<RecvStream>>),
    /// 协议数据升级, 第一参数表示将要写给客户端的消息, 第二参数表示原来未处理的请求
    ClientUpgradeHttp2(Settings),
    /// 发生错误或者收到关闭消息将要关闭该链接
    GoAway(Binary, Reason, Initiator),
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initiator {
    User,
    Library,
    Remote,
}


impl Display for ProtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtError::IoError(_) => f.write_str("io error"),
            ProtError::WebError(w) => w.fmt(f),
            ProtError::GoAway(_, _, _) => f.write_str("go away frame"),
            ProtError::Extension(s) => f.write_fmt(format_args!("extension {}", s)),
            ProtError::ServerUpgradeHttp2(_, _) => f.write_str("receive server upgrade http2 info"),
            ProtError::ClientUpgradeHttp2(_) => f.write_str("receive client upgrade http2 info"),
            ProtError::SendError => f.write_str("send erorr"),
        }
    }
}

impl From<io::Error>  for ProtError {
    fn from(value: io::Error) -> Self {
        ProtError::IoError(value)
    }
}


impl From<WebError>  for ProtError {
    fn from(value: WebError) -> Self {
        ProtError::WebError(value)
    }
}

impl<T> From<SendError<T>> for ProtError {
    fn from(_: SendError<T>) -> Self {
        ProtError::SendError
    }
}

unsafe impl Send for ProtError {
    
}

unsafe impl Sync for ProtError {
    
}

impl ProtError {
    pub(crate) fn library_go_away(reason: Reason) -> Self {
        Self::GoAway(Binary::new(), reason, Initiator::Library)
    }
}
