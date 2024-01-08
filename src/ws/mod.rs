mod codec;
mod control;
mod server_connection;
mod client_connection;
mod state;
mod ws_trait;
mod handshake;
mod option;

pub use codec::{FramedRead, FramedWrite, WsCodec};
use control::Control;
use state::{WsStateGoAway, WsStateHandshake, WsStatePingPong};
pub use client_connection::ClientWsConnection;
pub use server_connection::ServerWsConnection;
pub use ws_trait::WsTrait;
pub use handshake::WsHandshake;
pub use option::WsOption;

use webparse::ws::CloseData;
#[derive(Debug)]
enum WsState {
    /// Currently open in a sane state
    Open,

    /// The codec must be flushed
    Closing(CloseData),

    /// In a closed state
    Closed(CloseData),
}

impl WsState {
    pub fn set_closing(&mut self, data: CloseData) {
        match self {
            WsState::Open => {
                *self = WsState::Closing(data);
            },
            _ => {}
        }
    }
    pub fn set_closed(&mut self, data: Option<CloseData>) {
        match self {
            WsState::Open => {
                *self = WsState::Closed(data.unwrap_or(CloseData::normal()));
            },
            WsState::Closing(data) => {
                *self = WsState::Closed(data.clone());
            },
            _ => {}
        }
    }
}
