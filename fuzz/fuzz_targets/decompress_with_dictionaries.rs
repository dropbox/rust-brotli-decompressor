#![no_main]

extern crate brotli_decompressor;
extern crate libfuzzer_sys;

use brotli_decompressor::{Decompressor, SliceWrapperMut, StandardAlloc};
use libfuzzer_sys::fuzz_target;
use std::io::Read;

// Splits the input into (serialized dictionary, raw dictionary, brotli
// stream) and decompresses with both dictionaries attached, exercising the
// serialized-dictionary parser, the compound-dictionary copy paths and the
// generalized (custom word/transform) dictionary-word path.
fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let serialized_len = (u16::from_le_bytes([data[0], data[1]]) as usize) % (data.len() - 2);
    let rest = &data[2..];
    let (serialized, rest) = rest.split_at(serialized_len);
    if rest.len() < 2 {
        return;
    }
    let raw_len = (u16::from_le_bytes([rest[0], rest[1]]) as usize) % (rest.len() - 1);
    let rest = &rest[2..];
    let (raw, stream) = rest.split_at(raw_len);

    let mut alloc = StandardAlloc::default();
    let mut reader = Decompressor::new(stream, 4096);
    let mut serialized_mem =
        <StandardAlloc as brotli_decompressor::Allocator<u8>>::alloc_cell(&mut alloc, serialized.len());
    serialized_mem.slice_mut().clone_from_slice(serialized);
    let _ = reader.attach_serialized_dictionary(serialized_mem);
    let mut raw_mem =
        <StandardAlloc as brotli_decompressor::Allocator<u8>>::alloc_cell(&mut alloc, raw.len());
    raw_mem.slice_mut().clone_from_slice(raw);
    let _ = reader.attach_dictionary(raw_mem);
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
});
