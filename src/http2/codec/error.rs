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

use std::{error, fmt};

use crate::ProtError;

/// Errors caused by sending a message
#[derive(Debug)]
#[allow(dead_code)]
pub enum SendError {
    Connection(ProtError),
    User(UserError),
}

/// Errors caused by users of the library
#[derive(Debug)]
#[allow(dead_code)]
pub enum UserError {
    /// The stream ID is no longer accepting frames.
    InactiveStreamId,

    /// The stream is not currently expecting a frame of this type.
    UnexpectedFrameType,

    /// The payload size is too big
    PayloadTooBig,

    /// The application attempted to initiate too many streams to remote.
    Rejected,

    /// The released capacity is larger than claimed capacity.
    ReleaseCapacityTooBig,

    /// The stream ID space is overflowed.
    ///
    /// A new connection is needed.
    OverflowedStreamId,

    /// Illegal headers, such as connection-specific headers.
    MalformedHeaders,

    /// Request submitted with relative URI.
    MissingUriSchemeAndAuthority,

    /// Calls `SendResponse::poll_reset` after having called `send_response`.
    PollResetAfterSendResponse,

    /// Calls `PingPong::send_ping` before receiving a pong.
    SendPingWhilePending,

    /// Tries to update local SETTINGS while ACK has not been received.
    SendSettingsWhilePending,

    /// Tries to send push promise to peer who has disabled server push
    PeerDisabledServerPush,
}

// ===== impl SendError =====

impl error::Error for SendError {}

impl fmt::Display for SendError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Connection(ref e) => e.fmt(fmt),
            Self::User(ref e) => e.fmt(fmt),
        }
    }
}

// impl From<io::Error> for SendError {
//     fn from(src: io::Error) -> Self {
//         Self::Connection(src.into())
//     }
// }

impl From<UserError> for SendError {
    fn from(src: UserError) -> Self {
        SendError::User(src)
    }
}

// ===== impl UserError =====

impl error::Error for UserError {}

impl fmt::Display for UserError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::UserError::*;

        fmt.write_str(match *self {
            InactiveStreamId => "inactive stream",
            UnexpectedFrameType => "unexpected frame type",
            PayloadTooBig => "payload too big",
            Rejected => "rejected",
            ReleaseCapacityTooBig => "release capacity too big",
            OverflowedStreamId => "stream ID overflowed",
            MalformedHeaders => "malformed headers",
            MissingUriSchemeAndAuthority => "request URI missing scheme and authority",
            PollResetAfterSendResponse => "poll_reset after send_response is illegal",
            SendPingWhilePending => "send_ping before received previous pong",
            SendSettingsWhilePending => "sending SETTINGS before received previous ACK",
            PeerDisabledServerPush => "sending PUSH_PROMISE to peer who disabled server push",
        })
    }
}
