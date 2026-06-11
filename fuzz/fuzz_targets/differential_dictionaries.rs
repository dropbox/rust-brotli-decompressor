#![no_main]
// Differential fuzzer against the reference C implementation (linked in by
// build.rs from a google/brotli checkout, with BROTLI_EXPERIMENTAL so that
// serialized shared dictionaries are supported). Build and run with:
//
//   cargo +nightly fuzz run differential_dictionaries --features c-compat
//
// Three properties are checked on every input:
//
// 1. Round trip: the fuzz input is turned into a *valid* serialized shared
//    dictionary, up to two raw dictionaries, and content referencing them.
//    Whatever stream the C encoder produces with those dictionaries attached,
//    the Rust decoder must reproduce the content byte-for-byte (the C decoder
//    is also run as a sanity check).
// 2. Attach agreement: for a mutated (usually invalid) serialized dictionary,
//    Rust attach_serialized_dictionary and C BrotliDecoderAttachDictionary
//    must agree on acceptance.
// 3. Verdict agreement: for mutated or truncated streams, the two decoders
//    (with identical dictionaries attached) must agree on success vs failure,
//    and on the output bytes when both succeed.

extern crate brotli_decompressor;
extern crate libfuzzer_sys;

use brotli_decompressor::{BrotliDecompressStream, BrotliResult, BrotliState, SliceWrapperMut,
                          StandardAlloc};
use brotli_decompressor::Allocator;
use libfuzzer_sys::fuzz_target;

const OUTPUT_CAP: usize = 1 << 22;

#[allow(non_snake_case)]
mod c {
    pub enum EncoderState {}
    pub enum DecoderState {}
    pub enum PreparedDictionary {}
    use std::os::raw::{c_int, c_void};
    pub const BROTLI_PARAM_QUALITY: c_int = 1;
    pub const BROTLI_PARAM_LGWIN: c_int = 2;
    pub const BROTLI_PARAM_LARGE_WINDOW: c_int = 6;
    pub const BROTLI_DECODER_PARAM_LARGE_WINDOW: c_int = 1;
    pub const BROTLI_OPERATION_FINISH: c_int = 2;
    pub const BROTLI_SHARED_DICTIONARY_RAW: c_int = 0;
    pub const BROTLI_SHARED_DICTIONARY_SERIALIZED: c_int = 1;
    pub const BROTLI_DECODER_RESULT_SUCCESS: c_int = 1;
    pub const BROTLI_DECODER_RESULT_NEEDS_MORE_OUTPUT: c_int = 3;
    extern "C" {
        pub fn BrotliEncoderCreateInstance(alloc: *const c_void,
                                           free: *const c_void,
                                           opaque: *mut c_void)
                                           -> *mut EncoderState;
        pub fn BrotliEncoderDestroyInstance(s: *mut EncoderState);
        pub fn BrotliEncoderSetParameter(s: *mut EncoderState, param: c_int, value: u32)
                                         -> c_int;
        pub fn BrotliEncoderPrepareDictionary(dict_type: c_int,
                                              size: usize,
                                              data: *const u8,
                                              quality: c_int,
                                              alloc: *const c_void,
                                              free: *const c_void,
                                              opaque: *mut c_void)
                                              -> *mut PreparedDictionary;
        pub fn BrotliEncoderDestroyPreparedDictionary(d: *mut PreparedDictionary);
        pub fn BrotliEncoderAttachPreparedDictionary(s: *mut EncoderState,
                                                     d: *const PreparedDictionary)
                                                     -> c_int;
        pub fn BrotliEncoderCompressStream(s: *mut EncoderState,
                                           op: c_int,
                                           avail_in: *mut usize,
                                           next_in: *mut *const u8,
                                           avail_out: *mut usize,
                                           next_out: *mut *mut u8,
                                           total_out: *mut usize)
                                           -> c_int;
        pub fn BrotliEncoderIsFinished(s: *mut EncoderState) -> c_int;
        pub fn BrotliDecoderCreateInstance(alloc: *const c_void,
                                           free: *const c_void,
                                           opaque: *mut c_void)
                                           -> *mut DecoderState;
        pub fn BrotliDecoderDestroyInstance(s: *mut DecoderState);
        pub fn BrotliDecoderSetParameter(s: *mut DecoderState, param: c_int, value: u32)
                                         -> c_int;
        pub fn BrotliDecoderAttachDictionary(s: *mut DecoderState,
                                             dict_type: c_int,
                                             size: usize,
                                             data: *const u8)
                                             -> c_int;
        pub fn BrotliDecoderDecompressStream(s: *mut DecoderState,
                                             avail_in: *mut usize,
                                             next_in: *mut *const u8,
                                             avail_out: *mut usize,
                                             next_out: *mut *mut u8,
                                             total_out: *mut usize)
                                             -> c_int;
    }
}

// Deterministic byte source over the fuzz input; keeps producing (counter
// based) bytes after the input runs out so the generator always terminates.
struct Gen<'a> {
    data: &'a [u8],
    pos: usize,
    fallback: u8,
}

impl<'a> Gen<'a> {
    fn new(data: &'a [u8]) -> Self {
        Gen { data, pos: 0, fallback: 0x5b }
    }
    fn u8(&mut self) -> u8 {
        if self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            b
        } else {
            self.fallback = self.fallback.wrapping_mul(31).wrapping_add(7);
            self.fallback
        }
    }
    fn u16(&mut self) -> u16 {
        (self.u8() as u16) | ((self.u8() as u16) << 8)
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.u8()).collect()
    }
}

fn varint(mut n: usize, out: &mut Vec<u8>) {
    loop {
        let b = (n & 127) as u8;
        n >>= 7;
        if n != 0 {
            out.push(b | 128);
        } else {
            out.push(b);
            return;
        }
    }
}

// Builds a valid serialized shared dictionary from fuzz-chosen parameters.
// Returns the blob, the words of every custom word list (for content
// generation) and the prefix bytes.
fn build_serialized(g: &mut Gen) -> (Vec<u8>, Vec<Vec<u8>>, Vec<u8>) {
    let mut blob = vec![0x91u8, 0x00];
    let prefix_len = (g.u16() as usize) % 3000;
    let prefix = g.bytes(prefix_len);
    varint(prefix.len(), &mut blob);
    blob.extend_from_slice(&prefix);

    let num_word_lists = (g.u8() % 3) as usize; // 0..=2
    let mut words = Vec::new();
    blob.push(num_word_lists as u8);
    for _ in 0..num_word_lists {
        let mut size_bits = [0u8; 32];
        for _ in 0..1 + (g.u8() % 3) {
            let length = 4 + (g.u8() as usize) % 12; // 4..=15
            size_bits[length] = 1 + g.u8() % 3; // 2..8 words per length
        }
        blob.extend_from_slice(&size_bits[4..32]);
        for length in 4..32usize {
            if size_bits[length] != 0 {
                for _ in 0..(1usize << size_bits[length]) {
                    let word = g.bytes(length);
                    blob.extend_from_slice(&word);
                    words.push(word);
                }
            }
        }
    }

    let num_transform_lists = (g.u8() % 3) as usize; // 0..=2
    blob.push(num_transform_lists as u8);
    for _ in 0..num_transform_lists {
        // Stringlet table: a few short stringlets plus the empty terminator.
        let mut stringlets: Vec<Vec<u8>> = Vec::new();
        for _ in 0..(g.u8() % 4) {
            let len = 1 + (g.u8() as usize) % 5;
            stringlets.push(g.bytes(len));
        }
        stringlets.push(Vec::new()); // terminator; also the empty fix
        let mut table = Vec::new();
        for s in stringlets.iter() {
            table.push(s.len() as u8);
            table.extend_from_slice(s);
        }
        blob.push(table.len() as u8);
        blob.push((table.len() >> 8) as u8);
        blob.extend_from_slice(&table);
        let empty_id = (stringlets.len() - 1) as u8;
        // Transforms: always include the plain identity so the encoder can
        // emit untransformed words; then a few fuzz-chosen ones.
        let allowed_types = [0u8, 1, 2, 5, 9, 10, 11, 12, 14, 20, 21, 22];
        let mut transforms = vec![(empty_id, 0u8, empty_id)];
        for _ in 0..(g.u8() % 5) {
            let ttype = allowed_types[(g.u8() as usize) % allowed_types.len()];
            let pid = (g.u8() as usize % stringlets.len()) as u8;
            let sid = (g.u8() as usize % stringlets.len()) as u8;
            transforms.push((pid, ttype, sid));
        }
        let has_params = transforms.iter().any(|t| t.1 == 21 || t.1 == 22);
        blob.push(transforms.len() as u8);
        for t in transforms.iter() {
            blob.push(t.0);
            blob.push(t.1);
            blob.push(t.2);
        }
        if has_params {
            for t in transforms.iter() {
                if t.1 == 21 || t.1 == 22 {
                    blob.push(g.u8());
                    blob.push(g.u8() & 0x3); // keep the shift parameter modest
                } else {
                    blob.push(0);
                    blob.push(0);
                }
            }
        }
    }

    if num_word_lists != 0 || num_transform_lists != 0 {
        let num_dictionaries = 1 + (g.u8() as usize) % 3; // 1..=3
        blob.push(num_dictionaries as u8);
        for _ in 0..num_dictionaries {
            blob.push((g.u8() as usize % (num_word_lists + 1)) as u8);
            blob.push((g.u8() as usize % (num_transform_lists + 1)) as u8);
        }
        if num_dictionaries > 1 && g.u8() & 1 != 0 {
            blob.push(1); // CONTEXT_ENABLED
            for _ in 0..64 {
                blob.push((g.u8() as usize % num_dictionaries) as u8);
            }
        } else {
            blob.push(0);
        }
    }
    (blob, words, prefix)
}

fn build_content(g: &mut Gen,
                 words: &[Vec<u8>],
                 prefix: &[u8],
                 raws: &[Vec<u8>])
                 -> Vec<u8> {
    let target = 64 + (g.u16() as usize) % 24000;
    let mut content = Vec::with_capacity(target + 64);
    while content.len() < target {
        match g.u8() % 8 {
            0..=2 if !words.is_empty() => {
                let w = &words[(g.u16() as usize) % words.len()];
                content.extend_from_slice(w);
            }
            3 | 4 if !prefix.is_empty() => {
                let start = (g.u16() as usize) % prefix.len();
                let len = 4 + (g.u8() as usize) % 60;
                let end = core::cmp::min(start + len, prefix.len());
                content.extend_from_slice(&prefix[start..end]);
            }
            5 | 6 if !raws.is_empty() => {
                let raw = &raws[(g.u8() as usize) % raws.len()];
                if !raw.is_empty() {
                    let start = (g.u16() as usize) % raw.len();
                    let len = 4 + (g.u8() as usize) % 100;
                    let end = core::cmp::min(start + len, raw.len());
                    content.extend_from_slice(&raw[start..end]);
                }
            }
            _ => {
                let len = 1 + (g.u8() as usize) % 24;
                let chunk = g.bytes(len);
                content.extend_from_slice(&chunk);
            }
        }
        content.push(b' ');
    }
    content
}

// Compresses content with the C encoder, the serialized dictionary first and
// then each raw chunk. Returns None if the encoder rejects a dictionary.
fn c_encode(serialized: Option<&[u8]>,
            raws: &[Vec<u8>],
            content: &[u8],
            quality: i32,
            lgwin: i32)
            -> Option<Vec<u8>> {
    unsafe {
        let enc = c::BrotliEncoderCreateInstance(core::ptr::null(),
                                                 core::ptr::null(),
                                                 core::ptr::null_mut());
        assert!(!enc.is_null());
        c::BrotliEncoderSetParameter(enc, c::BROTLI_PARAM_QUALITY, quality as u32);
        if lgwin > 24 {
            c::BrotliEncoderSetParameter(enc, c::BROTLI_PARAM_LARGE_WINDOW, 1);
        }
        c::BrotliEncoderSetParameter(enc, c::BROTLI_PARAM_LGWIN, lgwin as u32);
        let mut prepared = Vec::new();
        let mut ok = true;
        let mut dicts: Vec<(i32, &[u8])> = Vec::new();
        if let Some(s) = serialized {
            dicts.push((c::BROTLI_SHARED_DICTIONARY_SERIALIZED, s));
        }
        for raw in raws.iter() {
            dicts.push((c::BROTLI_SHARED_DICTIONARY_RAW, &raw[..]));
        }
        for (dict_type, data) in dicts.into_iter() {
            let p = c::BrotliEncoderPrepareDictionary(dict_type,
                                                      data.len(),
                                                      data.as_ptr(),
                                                      quality,
                                                      core::ptr::null(),
                                                      core::ptr::null(),
                                                      core::ptr::null_mut());
            if p.is_null() {
                ok = false;
                break;
            }
            prepared.push(p);
            if c::BrotliEncoderAttachPreparedDictionary(enc, p) == 0 {
                ok = false;
                break;
            }
        }
        let result = if ok {
            let mut out = vec![0u8; content.len() + (content.len() >> 1) + 4096];
            let mut avail_in = content.len();
            let mut next_in = content.as_ptr();
            let mut avail_out = out.len();
            let mut next_out = out.as_mut_ptr();
            let mut success = true;
            loop {
                if c::BrotliEncoderCompressStream(enc,
                                                  c::BROTLI_OPERATION_FINISH,
                                                  &mut avail_in,
                                                  &mut next_in,
                                                  &mut avail_out,
                                                  &mut next_out,
                                                  core::ptr::null_mut()) == 0 {
                    success = false;
                    break;
                }
                if c::BrotliEncoderIsFinished(enc) != 0 {
                    break;
                }
            }
            if success {
                let written = out.len() - avail_out;
                out.truncate(written);
                Some(out)
            } else {
                None
            }
        } else {
            None
        };
        c::BrotliEncoderDestroyInstance(enc);
        for p in prepared.into_iter() {
            c::BrotliEncoderDestroyPreparedDictionary(p);
        }
        result
    }
}

enum DecodeResult {
    Success(Vec<u8>),
    Failure,
    AttachRejected,
    OutputCapHit,
}

fn c_decode(serialized: Option<&[u8]>, raws: &[Vec<u8>], stream: &[u8]) -> DecodeResult {
    unsafe {
        let dec = c::BrotliDecoderCreateInstance(core::ptr::null(),
                                                 core::ptr::null(),
                                                 core::ptr::null_mut());
        assert!(!dec.is_null());
        c::BrotliDecoderSetParameter(dec, c::BROTLI_DECODER_PARAM_LARGE_WINDOW, 1);
        let mut attach_ok = true;
        if let Some(s) = serialized {
            attach_ok &= c::BrotliDecoderAttachDictionary(
                dec, c::BROTLI_SHARED_DICTIONARY_SERIALIZED, s.len(), s.as_ptr()) != 0;
        }
        for raw in raws.iter() {
            attach_ok &= c::BrotliDecoderAttachDictionary(
                dec, c::BROTLI_SHARED_DICTIONARY_RAW, raw.len(), raw.as_ptr()) != 0;
        }
        let result = if !attach_ok {
            DecodeResult::AttachRejected
        } else {
            let mut out = vec![0u8; OUTPUT_CAP];
            let mut avail_in = stream.len();
            let mut next_in = stream.as_ptr();
            let mut avail_out = out.len();
            let mut next_out = out.as_mut_ptr();
            let r = c::BrotliDecoderDecompressStream(dec,
                                                     &mut avail_in,
                                                     &mut next_in,
                                                     &mut avail_out,
                                                     &mut next_out,
                                                     core::ptr::null_mut());
            if r == c::BROTLI_DECODER_RESULT_SUCCESS {
                let written = out.len() - avail_out;
                out.truncate(written);
                DecodeResult::Success(out)
            } else if r == c::BROTLI_DECODER_RESULT_NEEDS_MORE_OUTPUT {
                DecodeResult::OutputCapHit
            } else {
                // ERROR, or NEEDS_MORE_INPUT with the whole stream provided.
                DecodeResult::Failure
            }
        };
        c::BrotliDecoderDestroyInstance(dec);
        result
    }
}

fn rust_decode(serialized: Option<&[u8]>, raws: &[Vec<u8>], stream: &[u8]) -> DecodeResult {
    let mut alloc = StandardAlloc::default();
    let mut state = BrotliState::new(StandardAlloc::default(),
                                     StandardAlloc::default(),
                                     StandardAlloc::default());
    let mut attach_ok = true;
    if let Some(s) = serialized {
        let mut mem = <StandardAlloc as Allocator<u8>>::alloc_cell(&mut alloc, s.len());
        mem.slice_mut().clone_from_slice(s);
        attach_ok &= state.attach_serialized_dictionary(mem);
    }
    for raw in raws.iter() {
        let mut mem = <StandardAlloc as Allocator<u8>>::alloc_cell(&mut alloc, raw.len());
        mem.slice_mut().clone_from_slice(&raw[..]);
        attach_ok &= state.attach_dictionary(mem);
    }
    if !attach_ok {
        return DecodeResult::AttachRejected;
    }
    let mut out = vec![0u8; OUTPUT_CAP];
    let mut avail_in = stream.len();
    let mut input_offset = 0usize;
    let mut avail_out = out.len();
    let mut output_offset = 0usize;
    let mut total_out = 0usize;
    match BrotliDecompressStream(&mut avail_in,
                                 &mut input_offset,
                                 stream,
                                 &mut avail_out,
                                 &mut output_offset,
                                 &mut out[..],
                                 &mut total_out,
                                 &mut state) {
        BrotliResult::ResultSuccess => {
            out.truncate(output_offset);
            DecodeResult::Success(out)
        }
        BrotliResult::NeedsMoreOutput => DecodeResult::OutputCapHit,
        // ResultFailure, or NeedsMoreInput with the whole stream provided.
        _ => DecodeResult::Failure,
    }
}

// The two decoders must agree given identical dictionaries and stream.
fn compare_decoders(serialized: Option<&[u8]>, raws: &[Vec<u8>], stream: &[u8], what: &str) {
    let c_result = c_decode(serialized, raws, stream);
    let rust_result = rust_decode(serialized, raws, stream);
    match (&c_result, &rust_result) {
        (&DecodeResult::Success(ref c_out), &DecodeResult::Success(ref rust_out)) => {
            assert_eq!(c_out, rust_out, "output mismatch ({})", what);
        }
        (&DecodeResult::Failure, &DecodeResult::Failure) => {}
        (&DecodeResult::AttachRejected, &DecodeResult::AttachRejected) => {}
        // If either side ran into the output cap the comparison is
        // inconclusive; skip rather than report.
        (&DecodeResult::OutputCapHit, _) | (_, &DecodeResult::OutputCapHit) => {}
        (c_r, rust_r) => {
            panic!("verdict mismatch ({}): C {} vs Rust {}\nstream: {:02x?}\nserialized dict: {:02x?}\nraw dicts: {:02x?}",
                   what, describe(c_r), describe(rust_r), stream, serialized, raws);
        }
    }
}

fn describe(r: &DecodeResult) -> &'static str {
    match *r {
        DecodeResult::Success(_) => "success",
        DecodeResult::Failure => "failure",
        DecodeResult::AttachRejected => "attach-rejected",
        DecodeResult::OutputCapHit => "output-cap",
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let mut g = Gen::new(data);
    let mode = g.u8();
    let use_serialized = mode & 1 != 0;
    let num_raw = ((mode >> 1) % 3) as usize; // 0..=2
    let quality = [1i32, 5, 9, 11][((mode >> 3) % 4) as usize];
    let lgwin = [10i32, 12, 16, 18, 22, 24, 26, 14][((mode >> 5) % 8) as usize];

    let (serialized, words, prefix) = if use_serialized {
        let (blob, words, prefix) = build_serialized(&mut g);
        (Some(blob), words, prefix)
    } else {
        (None, Vec::new(), Vec::new())
    };
    let mut raws = Vec::new();
    for _ in 0..num_raw {
        let len = 1 + (g.u16() as usize) % 8192;
        raws.push(g.bytes(len));
    }
    let content = build_content(&mut g, &words, &prefix, &raws);

    // Property 1: anything the C encoder emits, the Rust decoder must decode
    // to the original content.
    if let Some(compressed) =
        c_encode(serialized.as_deref(), &raws, &content, quality, lgwin) {
        match c_decode(serialized.as_deref(), &raws, &compressed) {
            DecodeResult::Success(c_out) => {
                assert_eq!(c_out, content, "C decoder failed its own round trip");
            }
            other => panic!("C decoder failed its own round trip: {}", describe(&other)),
        }
        match rust_decode(serialized.as_deref(), &raws, &compressed) {
            DecodeResult::Success(rust_out) => {
                assert_eq!(rust_out, content, "Rust output differs from content");
            }
            other => panic!("Rust decoder rejected a C-encoded stream: {}", describe(&other)),
        }

        // Property 3a: mutated streams must produce identical verdicts.
        let mut mutated = compressed.clone();
        let pos = (g.u16() as usize) % mutated.len();
        mutated[pos] ^= 1 + g.u8() % 255;
        compare_decoders(serialized.as_deref(), &raws, &mutated, "mutated stream");

        // Property 3b: truncated streams must produce identical verdicts.
        let cut = (g.u16() as usize) % compressed.len();
        compare_decoders(serialized.as_deref(), &raws, &compressed[..cut], "truncated stream");
    }

    // Property 2: a mutated serialized dictionary must be accepted/rejected
    // identically (and when accepted by both, decoding must still agree).
    if let Some(ref blob) = serialized {
        let mut mutated = blob.clone();
        let pos = (g.u16() as usize) % mutated.len();
        mutated[pos] = mutated[pos].wrapping_add(1 + g.u8() % 255);
        let tail = g.bytes(64);
        compare_decoders(Some(&mutated[..]), &raws, &tail[..], "mutated dictionary");
    }
});
