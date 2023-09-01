
use webparse::{Buf, Binary};
use super::Builder;


pub struct Handshake { 
    /// 默认参数
    builder: Builder,
    /// 当前握手状态
    state: Handshaking,
    /// 握手日志信息
    span: tracing::Span,
}

/// 握手状态
enum Handshaking {
    /// 等待写入Settings帧
    Flushing,
    /// 等待读取Magic信息
    ReadingPreface,
    /// 已完成握手, 不可重复握手
    Done,
}