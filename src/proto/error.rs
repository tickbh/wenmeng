use std::{fmt::Display, io};

use webparse::WebError;

pub type ProtoResult<T> = Result<T, ProtoError>;

#[derive(Debug)]
pub enum ProtoError {
    IoError(io::Error),
    WebError(WebError),
    Extension(&'static str),
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

unsafe impl Send for ProtoError {
    
}

unsafe impl Sync for ProtoError {
    
}