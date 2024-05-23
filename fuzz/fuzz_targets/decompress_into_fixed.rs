#![no_main]

extern crate libfuzzer_sys;

use std::io::Cursor;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut input = Cursor::new(data);
    let mut output_buf = [0u8; 2048];
    let mut output = Cursor::new(&mut output_buf[..]);
    let _ = brotli_decompressor::BrotliDecompress(&mut input, &mut output);
});
