use algorithm::buf::BinaryMut;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io;
use std::io::prelude::*;

// Compress a sample string and print it after transformation.
fn main() {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    for i in 0..100 {
        e.write_all(format!(";Hello World {}", i).as_bytes())
            .unwrap();
    }
    let bytes = e.finish().unwrap();
    println!("{}", decode_reader(bytes).unwrap());
}

// Uncompresses a Gz Encoded vector of bytes and returns a string or error
// Here &[u8] implements BufRead
fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
    let mut bin = BinaryMut::new();
    bin.put_slice(&bytes[..100]);
    let mut gz = GzDecoder::new(bin);
    let mut s = String::new();
    let v = gz.read_to_string(&mut s);
    println!("s === {:?} v = {:?}", s, v);

    gz.write(&bytes[100..])?;
    let mut s = String::new();
    gz.read_to_string(&mut s)?;
    Ok(s)
}
