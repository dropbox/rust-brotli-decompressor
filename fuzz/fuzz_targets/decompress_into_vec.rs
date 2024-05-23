#![no_main]

extern crate libfuzzer_sys;

use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut input = Cursor::new(data);
    let mut output = Cursor::new(Vec::new());
    let _ = brotli_decompressor::BrotliDecompress(&mut input, &mut output);
});
