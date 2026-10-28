#![cfg(all(feature = "ffi-api", feature = "std"))]

// The one-shot decoder and the decoder state of a decoder created without C
// allocator callbacks come from the Rust global allocator.  A failed
// allocation there must surface as an error, as a C allocator returning null
// does, rather than abort the process.  This file is its own test binary, so
// the failing global allocator below affects nothing else.

extern crate brotli_decompressor;

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use brotli_decompressor::ffi::interface::BrotliDecoderResult;
use brotli_decompressor::ffi::{
  BrotliDecoderCreateInstance, BrotliDecoderDecompress, BrotliDecoderDecompressWithReturnInfo,
  BrotliDecoderDestroyInstance, BrotliDecoderErrorCode, BrotliDecoderState,
};
use brotli_decompressor::BrotliResult;

// Every allocation of at least this many bytes fails by returning null.  The
// tests run one at a time because they share it.
static ALLOCATION_LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);

fn failing(size: usize) -> bool {
  size >= ALLOCATION_LIMIT.load(Ordering::SeqCst)
}

struct LimitedAllocator;

unsafe impl GlobalAlloc for LimitedAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    if failing(layout.size()) {
      return ptr::null_mut();
    }
    System.alloc(layout)
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    if failing(layout.size()) {
      return ptr::null_mut();
    }
    System.alloc_zeroed(layout)
  }

  unsafe fn dealloc(&self, allocation: *mut u8, layout: Layout) {
    System.dealloc(allocation, layout)
  }

  unsafe fn realloc(&self, allocation: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    if failing(new_size) {
      return ptr::null_mut();
    }
    System.realloc(allocation, layout, new_size)
  }
}

#[global_allocator]
static GLOBAL: LimitedAllocator = LimitedAllocator;

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const ALICE: &'static [u8] = include_bytes!("../testdata/alice29.txt.compressed");
const ALICE_SIZE: usize = 152089;

struct Limit;

impl Limit {
  fn set(size: usize) -> Limit {
    ALLOCATION_LIMIT.store(size, Ordering::SeqCst);
    Limit
  }
}

impl Drop for Limit {
  fn drop(&mut self) {
    ALLOCATION_LIMIT.store(usize::MAX, Ordering::SeqCst);
  }
}

#[test]
fn one_shot_decodes_without_a_limit() {
  let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
  let mut output = vec![0u8; ALICE_SIZE];
  let mut size = output.len();
  let result = unsafe {
    BrotliDecoderDecompress(ALICE.len(), ALICE.as_ptr(), &mut size, output.as_mut_ptr())
  };
  assert_eq!(result as i32, BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS as i32);
  assert_eq!(size, ALICE_SIZE);
}

// The decoder's tables and ring buffer for this stream are all above 4 KiB,
// so each fails.  Allocation failure used to abort inside vec!.
#[test]
fn one_shot_allocation_failure_is_an_error_not_an_abort() {
  let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
  let mut output = vec![0u8; ALICE_SIZE];
  let _limit = Limit::set(4096);
  let mut size = output.len();
  let result = unsafe {
    BrotliDecoderDecompress(ALICE.len(), ALICE.as_ptr(), &mut size, output.as_mut_ptr())
  };
  assert_eq!(result as i32, BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR as i32);
  assert_eq!(size, 0);
}

#[test]
fn one_shot_allocation_failure_reports_the_alloc_error_code() {
  let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
  let mut output = vec![0u8; ALICE_SIZE];
  let _limit = Limit::set(4096);
  let info = unsafe {
    BrotliDecoderDecompressWithReturnInfo(ALICE.len(), ALICE.as_ptr(), output.len(),
                                          output.as_mut_ptr())
  };
  match info.result {
    BrotliResult::ResultFailure => {}
    _ => panic!("expected failure"),
  }
  let code = info.error_code as i32;
  assert!(code == BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_TREE_GROUPS as i32
          || code == BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MAP as i32
          || code == BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_1 as i32
          || code == BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_2 as i32
          || code == BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_BLOCK_TYPE_TREES as i32,
          "unexpected error code {}", code);
}

// Failing exactly the allocation of the state, which is larger than any table
// created with it, must make creation fail.
#[test]
fn create_instance_state_allocation_failure_returns_null() {
  let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
  let state_size = std::mem::size_of::<BrotliDecoderState>();
  {
    let _limit = Limit::set(state_size);
    let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
    assert!(state.is_null());
  }
  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());
  unsafe { BrotliDecoderDestroyInstance(state) };
}
