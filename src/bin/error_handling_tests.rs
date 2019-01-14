#![cfg(test)]
#![cfg(feature="std")]
use std::io::{self, Read};

extern crate brotli_decompressor;

static ENCODED: &[u8] = b"\x1b\x03)\x00\xa4\xcc\xde\xe2\xb3 vA\x00\x0c";

enum State { First, Second, Third, Fourth }
struct R(State);
impl Read for R {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0 {
            State::First => {
                buf[0] = ENCODED[0];
                self.0 = State::Second;
                return Ok(1);
            }
            State::Second => {
                self.0 = State::Third;
                return Err(io::Error::new(io::ErrorKind::WouldBlock, "foo"));
            }
            State::Third => {
                self.0 = State::Fourth;
                buf[..ENCODED.len() - 1].copy_from_slice(&ENCODED[1..]);
                return Ok(ENCODED[1..].len());
            }
            State::Fourth => {
                return Ok(0);
            }
        }
    }
}
#[test]
fn test_would_block() {

    let mut d = brotli_decompressor::Decompressor::new(R(State::First), 8192);
    let mut b = [0; 8192];
    assert_eq!(d.read(&mut b).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert!(d.read(&mut b).unwrap() != 0);
    println!("{}", String::from_utf8(b.to_vec()).unwrap());
    assert!(d.read(&mut b).unwrap() != 0);
    assert_eq!(d.read(&mut b).unwrap(), 0);
}
