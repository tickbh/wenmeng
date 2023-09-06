
mod server;
mod state;
mod codec;
mod connection;
mod control;
mod send_response;
mod inner_stream;
mod builder;
mod recv_stream;
mod priority_queue;

pub use priority_queue::PriorityQueue;
pub use recv_stream::RecvStream;
pub use inner_stream::InnerStream;
pub use send_response::{SendResponse, SendControl};
pub use control::Control;
pub use connection::Connection;
// pub use server::Builder;
pub use state::*;
pub use builder::Builder;

pub type FrameSize = u32;
pub type WindowSize = u32;

// Constants
pub const MAX_WINDOW_SIZE: WindowSize = (1 << 31) - 1; // i32::MAX as u32
pub const DEFAULT_REMOTE_RESET_STREAM_MAX: usize = 20;
pub const DEFAULT_RESET_STREAM_MAX: usize = 10;
pub const DEFAULT_RESET_STREAM_SECS: u64 = 30;
pub const DEFAULT_MAX_SEND_BUFFER_SIZE: usize = 1024 * 400;



/// The default value of SETTINGS_HEADER_TABLE_SIZE
pub const DEFAULT_SETTINGS_HEADER_TABLE_SIZE: usize = 4_096;

/// The default value of SETTINGS_INITIAL_WINDOW_SIZE
pub const DEFAULT_INITIAL_WINDOW_SIZE: u32 = 65_535;

/// The default value of MAX_FRAME_SIZE
pub const DEFAULT_MAX_FRAME_SIZE: FrameSize = 16_384;

/// INITIAL_WINDOW_SIZE upper bound
pub const MAX_INITIAL_WINDOW_SIZE: usize = (1 << 31) - 1;

/// MAX_FRAME_SIZE upper bound
pub const MAX_MAX_FRAME_SIZE: FrameSize = (1 << 24) - 1;