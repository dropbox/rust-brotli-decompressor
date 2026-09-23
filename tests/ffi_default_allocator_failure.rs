#![cfg(all(feature = "ffi-api", feature = "std"))]

// A decoder created without C allocator callbacks allocates from the Rust
// global allocator. A failed allocation there must surface as a decoder error,
// just as a C allocator returning null does, rather than abort the process.
// This file is its own test binary, so the failing global allocator below
// affects nothing else.

extern crate brotli_decompressor;

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr;

use brotli_decompressor::ffi::interface::BrotliDecoderResult;
use brotli_decompressor::ffi::{
  BrotliDecoderCreateInstance, BrotliDecoderDecompressStream, BrotliDecoderDestroyInstance,
  BrotliDecoderErrorCode, BrotliDecoderFreeU8, BrotliDecoderGetErrorCode, BrotliDecoderMallocU8,
  BrotliDecoderMallocUsize, BrotliDecoderState,
};

// Every allocation of at least this many bytes fails by returning null, the
// way an allocator under memory pressure does. Only the ring buffer for a
// 16 MiB window, and the explicit requests below, are that large.
const ALLOCATION_LIMIT: usize = 8 << 20;

struct LimitedAllocator;

unsafe impl GlobalAlloc for LimitedAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    if layout.size() >= ALLOCATION_LIMIT {
      return ptr::null_mut();
    }
    System.alloc(layout)
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    if layout.size() >= ALLOCATION_LIMIT {
      return ptr::null_mut();
    }
    System.alloc_zeroed(layout)
  }

  unsafe fn dealloc(&self, allocation: *mut u8, layout: Layout) {
    System.dealloc(allocation, layout)
  }

  unsafe fn realloc(&self, allocation: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    if new_size >= ALLOCATION_LIMIT {
      return ptr::null_mut();
    }
    System.realloc(allocation, layout, new_size)
  }
}

#[global_allocator]
static GLOBAL: LimitedAllocator = LimitedAllocator;

const EXPECTED: &'static [u8] = b"0123456789abcdefghij";

// Two 10-byte uncompressed meta-blocks followed by an empty last one, with a
// 1 << window_bits byte window. The first meta-block is not the last, so the
// decoder sizes its ring buffer to the whole window.
fn two_block_stream(window_bits: u8) -> Vec<u8> {
  assert!(window_bits >= 18 && window_bits <= 24);
  // WBITS, then ISLAST=0, MNIBBLES=4, MLEN-1=9, ISUNCOMPRESSED=1.
  let mut stream = vec![0x81 | ((window_bits - 17) << 1), 0x04, 0x80];
  stream.extend_from_slice(&EXPECTED[..10]);
  // Byte aligned: ISLAST=0, MNIBBLES=4, MLEN-1=9, ISUNCOMPRESSED=1.
  stream.extend_from_slice(&[0x48, 0x00, 0x08]);
  stream.extend_from_slice(&EXPECTED[10..]);
  // ISLAST=1, ISLASTEMPTY=1.
  stream.push(0x03);
  stream
}

unsafe fn decompress(
  state: *mut BrotliDecoderState,
  input: &[u8],
) -> (BrotliDecoderResult, Vec<u8>) {
  let mut output = vec![0u8; EXPECTED.len() + 1];
  let mut available_in = input.len();
  let mut next_in = input.as_ptr();
  let mut available_out = output.len();
  let mut next_out = output.as_mut_ptr();
  let mut total_out = 0usize;
  let result = BrotliDecoderDecompressStream(
    state,
    &mut available_in,
    &mut next_in,
    &mut available_out,
    &mut next_out,
    &mut total_out,
  );
  output.truncate(total_out);
  (result, output)
}

#[test]
fn default_allocator_decodes_when_the_ring_buffer_fits() {
  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());
  let (result, output) = unsafe { decompress(state, &two_block_stream(22)) };
  assert_eq!(
    result as i32,
    BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS as i32
  );
  assert_eq!(&output[..], EXPECTED);
  unsafe { BrotliDecoderDestroyInstance(state) };
}

// The 16 MiB window needs a ring buffer above the allocation limit. This used
// to abort the process inside vec! (via handle_alloc_error, which catch_unwind
// cannot intercept) at the ring buffer allocation in BrotliDecompressStream.
#[test]
fn default_allocator_ring_buffer_failure_is_an_error_not_an_abort() {
  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());
  let input = two_block_stream(24);
  // The failure is sticky, so a retry must fail the same way.
  for _ in 0..2 {
    let (result, output) = unsafe { decompress(state, &input) };
    assert_eq!(
      result as i32,
      BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR as i32
    );
    assert!(output.is_empty());
    assert_eq!(
      unsafe { BrotliDecoderGetErrorCode(state) } as i32,
      BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_2 as i32
    );
  }
  unsafe { BrotliDecoderDestroyInstance(state) };
}

#[test]
fn default_allocator_malloc_returns_null_on_failure() {
  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());
  unsafe {
    assert!(BrotliDecoderMallocU8(state, ALLOCATION_LIMIT).is_null());
    assert!(BrotliDecoderMallocUsize(state, ALLOCATION_LIMIT).is_null());
    let small = BrotliDecoderMallocU8(state, 16);
    assert!(!small.is_null());
    assert_eq!(std::slice::from_raw_parts(small, 16), &[0u8; 16]);
    BrotliDecoderFreeU8(state, small, 16);
    BrotliDecoderDestroyInstance(state);
  }
}
