use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::{Buf, http::http2::frame::Settings};

use super::handshake::Handshake;



#[derive(Clone, Debug)]
pub struct Builder {
    /// Time to keep locally reset streams around before reaping.
    reset_stream_duration: Duration,

    /// Maximum number of locally reset streams to keep at a time.
    reset_stream_max: usize,

    /// Maximum number of remotely reset streams to allow in the pending
    /// accept queue.
    pending_accept_reset_stream_max: usize,

    /// Initial `Settings` frame to send as part of the handshake.
    settings: Settings,

    /// Initial target window size for new connections.
    initial_target_connection_window_size: Option<u32>,

    /// Maximum amount of bytes to "buffer" for writing per stream.
    max_send_buffer_size: usize,
}



impl Builder {
    /// Returns a new server builder instance initialized with default
    /// configuration values.
    ///
    /// Configuration methods can be chained on the return value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .initial_window_size(1_000_000)
    ///     .max_concurrent_streams(1000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn new() -> Builder {
        Builder {
            reset_stream_duration: Duration::from_secs(super::DEFAULT_RESET_STREAM_SECS),
            reset_stream_max: super::DEFAULT_RESET_STREAM_MAX,
            pending_accept_reset_stream_max: super::DEFAULT_REMOTE_RESET_STREAM_MAX,
            settings: Settings::default(),
            initial_target_connection_window_size: None,
            max_send_buffer_size: super::DEFAULT_MAX_SEND_BUFFER_SIZE,
        }
    }

    /// Indicates the initial window size (in octets) for stream-level
    /// flow control for received data.
    ///
    /// The initial window of a stream is used as part of flow control. For more
    /// details, see [`FlowControl`].
    ///
    /// The default value is 65,535.
    ///
    /// [`FlowControl`]: ../struct.FlowControl.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .initial_window_size(1_000_000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn initial_window_size(&mut self, size: u32) -> &mut Self {
        self.settings.set_initial_window_size(Some(size));
        self
    }

    /// Indicates the initial window size (in octets) for connection-level flow control
    /// for received data.
    ///
    /// The initial window of a connection is used as part of flow control. For more details,
    /// see [`FlowControl`].
    ///
    /// The default value is 65,535.
    ///
    /// [`FlowControl`]: ../struct.FlowControl.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .initial_connection_window_size(1_000_000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn initial_connection_window_size(&mut self, size: u32) -> &mut Self {
        self.initial_target_connection_window_size = Some(size);
        self
    }

    /// Indicates the size (in octets) of the largest HTTP/2 frame payload that the
    /// configured server is able to accept.
    ///
    /// The sender may send data frames that are **smaller** than this value,
    /// but any data larger than `max` will be broken up into multiple `DATA`
    /// frames.
    ///
    /// The value **must** be between 16,384 and 16,777,215. The default value is 16,384.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_frame_size(1_000_000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if `max` is not within the legal range specified
    /// above.
    pub fn max_frame_size(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_frame_size(Some(max));
        self
    }

    /// Sets the max size of received header frames.
    ///
    /// This advisory setting informs a peer of the maximum size of header list
    /// that the sender is prepared to accept, in octets. The value is based on
    /// the uncompressed size of header fields, including the length of the name
    /// and value in octets plus an overhead of 32 octets for each header field.
    ///
    /// This setting is also used to limit the maximum amount of data that is
    /// buffered to decode HEADERS frames.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_header_list_size(16 * 1024)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_header_list_size(Some(max));
        self
    }

    /// Sets the maximum number of concurrent streams.
    ///
    /// The maximum concurrent streams setting only controls the maximum number
    /// of streams that can be initiated by the remote peer. In other words,
    /// when this setting is set to 100, this does not limit the number of
    /// concurrent streams that can be created by the caller.
    ///
    /// It is recommended that this value be no smaller than 100, so as to not
    /// unnecessarily limit parallelism. However, any value is legal, including
    /// 0. If `max` is set to 0, then the remote will not be permitted to
    /// initiate streams.
    ///
    /// Note that streams in the reserved state, i.e., push promises that have
    /// been reserved but the stream has not started, do not count against this
    /// setting.
    ///
    /// Also note that if the remote *does* exceed the value set here, it is not
    /// a protocol level error. Instead, the `h2` library will immediately reset
    /// the stream.
    ///
    /// See [Section 5.1.2] in the HTTP/2 spec for more details.
    ///
    /// [Section 5.1.2]: https://http2.github.io/http2-spec/#rfc.section.5.1.2
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_concurrent_streams(1000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_concurrent_streams(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_concurrent_streams(Some(max));
        self
    }

    /// Sets the maximum number of concurrent locally reset streams.
    ///
    /// When a stream is explicitly reset by either calling
    /// [`SendResponse::send_reset`] or by dropping a [`SendResponse`] instance
    /// before completing the stream, the HTTP/2 specification requires that
    /// any further frames received for that stream must be ignored for "some
    /// time".
    ///
    /// In order to satisfy the specification, internal state must be maintained
    /// to implement the behavior. This state grows linearly with the number of
    /// streams that are locally reset.
    ///
    /// The `max_concurrent_reset_streams` setting configures sets an upper
    /// bound on the amount of state that is maintained. When this max value is
    /// reached, the oldest reset stream is purged from memory.
    ///
    /// Once the stream has been fully purged from memory, any additional frames
    /// received for that stream will result in a connection level protocol
    /// error, forcing the connection to terminate.
    ///
    /// The default value is 10.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_concurrent_reset_streams(1000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_concurrent_reset_streams(&mut self, max: usize) -> &mut Self {
        self.reset_stream_max = max;
        self
    }

    /// Sets the maximum number of pending-accept remotely-reset streams.
    ///
    /// Streams that have been received by the peer, but not accepted by the
    /// user, can also receive a RST_STREAM. This is a legitimate pattern: one
    /// could send a request and then shortly after, realize it is not needed,
    /// sending a CANCEL.
    ///
    /// However, since those streams are now "closed", they don't count towards
    /// the max concurrent streams. So, they will sit in the accept queue,
    /// using memory.
    ///
    /// When the number of remotely-reset streams sitting in the pending-accept
    /// queue reaches this maximum value, a connection error with the code of
    /// `ENHANCE_YOUR_CALM` will be sent to the peer, and returned by the
    /// `Future`.
    ///
    /// The default value is currently 20, but could change.
    ///
    /// # Examples
    ///
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_pending_accept_reset_streams(100)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_pending_accept_reset_streams(&mut self, max: usize) -> &mut Self {
        self.pending_accept_reset_stream_max = max;
        self
    }

    /// Sets the maximum send buffer size per stream.
    ///
    /// Once a stream has buffered up to (or over) the maximum, the stream's
    /// flow control will not "poll" additional capacity. Once bytes for the
    /// stream have been written to the connection, the send buffer capacity
    /// will be freed up again.
    ///
    /// The default is currently ~400MB, but may change.
    ///
    /// # Panics
    ///
    /// This function panics if `max` is larger than `u32::MAX`.
    pub fn max_send_buffer_size(&mut self, max: usize) -> &mut Self {
        assert!(max <= std::u32::MAX as usize);
        self.max_send_buffer_size = max;
        self
    }

    /// Sets the maximum number of concurrent locally reset streams.
    ///
    /// When a stream is explicitly reset by either calling
    /// [`SendResponse::send_reset`] or by dropping a [`SendResponse`] instance
    /// before completing the stream, the HTTP/2 specification requires that
    /// any further frames received for that stream must be ignored for "some
    /// time".
    ///
    /// In order to satisfy the specification, internal state must be maintained
    /// to implement the behavior. This state grows linearly with the number of
    /// streams that are locally reset.
    ///
    /// The `reset_stream_duration` setting configures the max amount of time
    /// this state will be maintained in memory. Once the duration elapses, the
    /// stream state is purged from memory.
    ///
    /// Once the stream has been fully purged from memory, any additional frames
    /// received for that stream will result in a connection level protocol
    /// error, forcing the connection to terminate.
    ///
    /// The default value is 30 seconds.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// # use std::time::Duration;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .reset_stream_duration(Duration::from_secs(10))
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn reset_stream_duration(&mut self, dur: Duration) -> &mut Self {
        self.reset_stream_duration = dur;
        self
    }

    /// Enables the [extended CONNECT protocol].
    ///
    /// [extended CONNECT protocol]: https://datatracker.ietf.org/doc/html/rfc8441#section-4
    pub fn enable_connect_protocol(&mut self) -> &mut Self {
        self.settings.set_enable_connect_protocol(Some(1));
        self
    }

    /// Creates a new configured HTTP/2 server backed by `io`.
    ///
    /// It is expected that `io` already be in an appropriate state to commence
    /// the [HTTP/2 handshake]. See [Handshake] for more details.
    ///
    /// Returns a future which resolves to the [`Connection`] instance once the
    /// HTTP/2 handshake has been completed.
    ///
    /// This function also allows the caller to configure the send payload data
    /// type. See [Outbound data type] for more details.
    ///
    /// [HTTP/2 handshake]: http://httpwg.org/specs/rfc7540.html#ConnectionHeader
    /// [Handshake]: ../index.html#handshake
    /// [`Connection`]: struct.Connection.html
    /// [Outbound data type]: ../index.html#outbound-data-type.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    ///
    /// Configures the send-payload data type. In this case, the outbound data
    /// type will be `&'static [u8]`.
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T, &'static [u8]>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut: Handshake<_, &'static [u8]> = Builder::new()
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn handshake() {

    }
    // pub fn handshake<T, B>(&self, io: T) -> Handshake<T, B>
    // where
    //     T: AsyncRead + AsyncWrite + Unpin,
    //     B: Buf,
    // {
    //     Connection::handshake2(io, self.clone())
    // }
}
