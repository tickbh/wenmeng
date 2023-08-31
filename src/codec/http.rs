use std::{io, fmt};

use bytes::{BytesMut, BufMut, Buf};

// use http::{header::HeaderValue, Request, Response, StatusCode};

use tokio_util::codec::{Encoder, Decoder};
use webparse::{Response, Request, Serialize, BinaryMut, http::{request::Parts, http2::Http2}, Version};

// http2协议保留头数据以做共享数据
pub struct Http(pub Option<Parts>);

/// Implementation of encoding an HTTP response into a `BytesMut`, basically
/// just writing out an HTTP/1.1 response.
impl Encoder<Response<String>> for Http {
    type Error = io::Error;

    fn encode(&mut self, mut item: Response<String>, dst: &mut BytesMut) -> io::Result<()> {
        let mut buf = BinaryMut::new();
        let _ = Http2::serialize(&mut item, &mut buf);
        // let _ = item.serialize(&mut buf);
        dst.put_slice(buf.as_slice_all());
        return Ok(());

    }
}

/// Implementation of decoding an HTTP request from the bytes we've read so far.
/// This leverages the `httparse` crate to do the actual parsing and then we use
/// that information to construct an instance of a `http::Request` object,
/// trying to avoid allocations where possible.
impl Decoder for Http {
    type Item = Request<()>;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Request<()>>> {
        if !src.has_remaining() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Interrupted"));
        }
        let (req, result) = if self.0.is_some() && self.0.as_ref().unwrap().version == Version::Http2 {
            let mut req = Request::new_by_parts(self.0.clone().unwrap());
            let result = req.parse2(src.chunk());
            (req, result)
        } else {
            let mut req = Request::new();
            let result = req.parse(src.chunk());
            (req, result)
        };
        match result {
            Ok(len) => {
                src.advance(len);
                if req.is_http2() {
                    self.0 = Some(req.parts().clone());
                }
                Ok(Some(req))
            }
            Err(err) => {
                println!("remain === {:?}", src.remaining());
                Err(io::Error::new(io::ErrorKind::Other, "partical"))
            }
        }
    }
}


mod date {
    use std::cell::RefCell;
    use std::fmt::{self, Write};
    use std::str;
    use std::time::SystemTime;

    use httpdate::HttpDate;

    pub struct Now(());

    /// Returns a struct, which when formatted, renders an appropriate `Date`
    /// header value.
    pub fn now() -> Now {
        Now(())
    }

    // Gee Alex, doesn't this seem like premature optimization. Well you see
    // there Billy, you're absolutely correct! If your server is *bottlenecked*
    // on rendering the `Date` header, well then boy do I have news for you, you
    // don't need this optimization.
    //
    // In all seriousness, though, a simple "hello world" benchmark which just
    // sends back literally "hello world" with standard headers actually is
    // bottlenecked on rendering a date into a byte buffer. Since it was at the
    // top of a profile, and this was done for some competitive benchmarks, this
    // module was written.
    //
    // Just to be clear, though, I was not intending on doing this because it
    // really does seem kinda absurd, but it was done by someone else [1], so I
    // blame them!  :)
    //
    // [1]: https://github.com/rapidoid/rapidoid/blob/f1c55c0555007e986b5d069fe1086e6d09933f7b/rapidoid-commons/src/main/java/org/rapidoid/commons/Dates.java#L48-L66

    struct LastRenderedNow {
        bytes: [u8; 128],
        amt: usize,
        unix_date: u64,
    }

    thread_local!(static LAST: RefCell<LastRenderedNow> = RefCell::new(LastRenderedNow {
        bytes: [0; 128],
        amt: 0,
        unix_date: 0,
    }));

    impl fmt::Display for Now {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            LAST.with(|cache| {
                let mut cache = cache.borrow_mut();
                let now = SystemTime::now();
                let now_unix = now
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|since_epoch| since_epoch.as_secs())
                    .unwrap_or(0);
                if cache.unix_date != now_unix {
                    cache.update(now, now_unix);
                }
                f.write_str(cache.buffer())
            })
        }
    }

    impl LastRenderedNow {
        fn buffer(&self) -> &str {
            str::from_utf8(&self.bytes[..self.amt]).unwrap()
        }

        fn update(&mut self, now: SystemTime, now_unix: u64) {
            self.amt = 0;
            self.unix_date = now_unix;
            write!(LocalBuffer(self), "{}", HttpDate::from(now)).unwrap();
        }
    }

    struct LocalBuffer<'a>(&'a mut LastRenderedNow);

    impl fmt::Write for LocalBuffer<'_> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let start = self.0.amt;
            let end = start + s.len();
            self.0.bytes[start..end].copy_from_slice(s.as_bytes());
            self.0.amt += s.len();
            Ok(())
        }
    }
}
