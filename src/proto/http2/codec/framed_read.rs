
use tokio_util::codec::FramedRead as InnerFramedRead;
use tokio_util::codec::{LengthDelimitedCodec, LengthDelimitedCodecError};
use webparse::BinaryMut;
use webparse::http::http2::HeaderIndex;

#[derive(Debug)]
pub struct FramedRead<T> {
    inner: InnerFramedRead<T, LengthDelimitedCodec>,

    // hpack decoder state
    head_index: HeaderIndex,

    max_header_list_size: usize,

    // partial: Option<Partial>,
}


// /// Partially loaded headers frame
// #[derive(Debug)]
// struct Partial {
//     /// Empty frame
//     frame: Continuable,

//     /// Partial header payload
//     buf: BinaryMut,
// }

// #[derive(Debug)]
// enum Continuable {
//     Headers(FrameHeader),
//     PushPromise(PushPromise),
// }