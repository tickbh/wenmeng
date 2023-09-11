use std::{fmt::{Display}, io};

use webparse::{WebError, Binary, http::http2::frame::Reason, BinaryMut};

pub type ProtoResult<T> = Result<T, ProtoError>;

#[derive(Debug)]
pub enum ProtoError {
    IoError(io::Error),
    WebError(WebError),
    SenderError(),
    GoAway(Binary, Reason, Initiator),
    Extension(&'static str),
    IsNotFull,
    UpgradeHttp2,
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initiator {
    User,
    Library,
    Remote,
}


impl Display for ProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<io::Error>  for ProtoError {
    fn from(value: io::Error) -> Self {
        ProtoError::IoError(value)
    }
}


impl From<WebError>  for ProtoError {
    fn from(value: WebError) -> Self {
        ProtoError::WebError(value)
    }
}

// impl<T> From<TrySendError<T>>  for ProtoError {
//     fn from(value: TrySendError<T>) -> Self {
//         ma
//         ProtoError::WebError(value)
//     }
// }

unsafe impl Send for ProtoError {
    
}

unsafe impl Sync for ProtoError {
    
}

impl ProtoError {
    pub(crate) fn library_go_away(reason: Reason) -> Self {
        Self::GoAway(Binary::new(), reason, Initiator::Library)
    }
}