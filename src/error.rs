// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
// 
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
// 
// Author: tickbh
// -----
// Created Date: 2023/09/14 09:42:25

use std::{fmt::{Display, Pointer}, io};

use tokio::sync::mpsc::error::SendError;
use webparse::{WebError, Binary, http::http2::frame::Reason, http2::frame::Settings};

use crate::{RecvRequest};

pub type ProtResult<T> = Result<T, ProtError>;

#[derive(Debug)]
pub enum TimeoutError {
    Connect(&'static str),
    Read(&'static str),
    Write(&'static str),
    Time(&'static str),
    KeepAlive(&'static str),
    Extension(&'static str)
}

impl TimeoutError {

    pub fn is_read(&self) -> (bool, bool) {
        match self {
            TimeoutError::Read(info) => (true, info == &"client"),
            _ => (false, false)
        }
    }

    pub fn is_write(&self) -> (bool, bool) {
        match self {
            TimeoutError::Write(info) => (true, info == &"client"),
            _ => (false, false)
        }
    }

    pub fn is_client(&self) -> bool {
        match self {
            TimeoutError::Connect(info) => info == &"client",
            TimeoutError::Read(info) => info == &"client",
            TimeoutError::Write(info) => info == &"client",
            TimeoutError::Time(info) => info == &"client",
            TimeoutError::KeepAlive(info) => info == &"client",
            TimeoutError::Extension(info) => info == &"client",
        }
    }
    
    pub fn is_server(&self) -> bool {
        match self {
            TimeoutError::Connect(info) => info == &"server",
            TimeoutError::Read(info) => info == &"server",
            TimeoutError::Write(info) => info == &"server",
            TimeoutError::Time(info) => info == &"server",
            TimeoutError::KeepAlive(info) => info == &"server",
            TimeoutError::Extension(info) => info == &"server",
        }
    }
}

#[derive(Debug)]
pub enum ProtError {
    /// 标准错误库的错误类型
    IoError(io::Error),
    /// 解析库发生错误
    WebError(WebError),
    /// 其它错误信息
    Extension(&'static str),
    Timeout(TimeoutError),

    SendError,
    /// 协议数据升级, 第一参数表示将要写给客户端的消息, 第二参数表示原来未处理的请求
    ServerUpgradeHttp2(Binary, Option<RecvRequest>),
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
            ProtError::Timeout(t) => t.fmt(f),
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

    pub fn is_timeout(&self) -> (bool, bool) {
        match self {
            Self::Timeout(timeout) => (true, timeout.is_client()),
            _ => (false, false),
        }
    }

    pub fn is_io(&self) -> bool {
        match self {
            Self::IoError(_) => true,
            _ => false,
        }
    }
    
    pub fn is_read_timeout(&self) -> (bool, bool) {
        match self {
            Self::Timeout(timeout) => timeout.is_read(),
            _ => (false, false),
        }
    }
    
    pub fn is_write_timeout(&self) -> (bool, bool) {
        match self {
            Self::Timeout(timeout) => timeout.is_read(),
            _ => (false, false),
        }
    }

    pub fn is_server_upgrade_http2(&self) -> bool {
        match self {
            Self::ServerUpgradeHttp2(_, _) => true,
            _ => false,
        }
    }

    pub fn connect_timeout(val: &'static str) -> Self {
        Self::Timeout(TimeoutError::Connect(val))
    }
    
    pub fn read_timeout(val: &'static str) -> Self {
        Self::Timeout(TimeoutError::Read(val))
    }
    
    pub fn write_timeout(val: &'static str) -> Self {
        Self::Timeout(TimeoutError::Write(val))
    }
    
    pub fn time_timeout(val: &'static str) -> Self {
        Self::Timeout(TimeoutError::Time(val))
    }
    
    pub fn ka_timeout(val: &'static str) -> Self {
        Self::Timeout(TimeoutError::KeepAlive(val))
    }
}
