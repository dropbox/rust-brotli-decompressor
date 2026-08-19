#![cfg(test)]
#![cfg(all(feature = "ffi-api", feature = "std"))]

use std::alloc::{alloc, dealloc, Layout};
use std::mem;
use std::ptr;
use std::vec::Vec;

use brotli_decompressor::ffi::interface::{c_void, BrotliDecoderResult};
use brotli_decompressor::ffi::{
  BrotliDecoderAttachDictionary, BrotliDecoderCreateInstance, BrotliDecoderDecompressStream,
  BrotliDecoderDestroyInstance, BrotliDecoderErrorCode, BrotliDecoderIsUsed,
};

struct FailingAllocator {
  fail_next: bool,
  allocations: Vec<(*mut u8, Layout)>,
}

extern "C" fn test_alloc(opaque: *mut c_void, size: usize) -> *mut c_void {
  let allocator = unsafe { &mut *(opaque as *mut FailingAllocator) };
  if allocator.fail_next {
    allocator.fail_next = false;
    return ptr::null_mut();
  }
  let layout = Layout::from_size_align(size, 64).unwrap();
  let allocation = unsafe { alloc(layout) };
  if !allocation.is_null() {
    allocator.allocations.push((allocation, layout));
  }
  allocation as *mut c_void
}

extern "C" fn test_free(opaque: *mut c_void, allocation: *mut c_void) {
  if allocation.is_null() {
    return;
  }
  let allocator = unsafe { &mut *(opaque as *mut FailingAllocator) };
  let allocation = allocation as *mut u8;
  let index = allocator.allocations.iter()
    .position(|&(candidate, _)| candidate == allocation)
    .expect("free of unknown test allocation");
  let (_, layout) = allocator.allocations.swap_remove(index);
  unsafe { dealloc(allocation, layout) };
}

#[test]
fn create_instance_returns_null_when_custom_allocator_is_exhausted() {
  let mut allocator = FailingAllocator {
    fail_next: true,
    allocations: Vec::new(),
  };
  let opaque = &mut allocator as *mut FailingAllocator as *mut c_void;
  let state = unsafe {
    BrotliDecoderCreateInstance(Some(test_alloc), Some(test_free), opaque)
  };

  assert!(state.is_null());
  assert!(allocator.allocations.is_empty());
}

#[test]
fn attach_dictionary_returns_false_when_custom_allocator_is_exhausted() {
  let mut allocator = FailingAllocator {
    fail_next: false,
    allocations: Vec::new(),
  };
  let opaque = &mut allocator as *mut FailingAllocator as *mut c_void;
  let state = unsafe {
    BrotliDecoderCreateInstance(Some(test_alloc), Some(test_free), opaque)
  };
  assert!(!state.is_null());

  allocator.fail_next = true;
  let dictionary = [0x61u8];
  assert_eq!(unsafe {
    BrotliDecoderAttachDictionary(state, 0, dictionary.len(), dictionary.as_ptr())
  }, 0);

  unsafe { BrotliDecoderDestroyInstance(state) };
  assert!(allocator.allocations.is_empty());
}

#[test]
fn stream_rejects_null_input_buffer_with_nonzero_length() {
  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());

  let mut available_in = 1usize;
  let mut input = ptr::null();
  let mut available_out = 0usize;
  let mut output = ptr::null_mut();
  let result = unsafe {
    BrotliDecoderDecompressStream(
      state,
      &mut available_in,
      &mut input,
      &mut available_out,
      &mut output,
      ptr::null_mut(),
    )
  };

  assert_eq!(
    result as i32,
    BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR as i32,
  );
  assert_eq!(
    unsafe { (*state).decompressor.error_code } as i32,
    BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS as i32,
  );
  unsafe { BrotliDecoderDestroyInstance(state) };
}

#[test]
fn stream_rejects_wrapping_input_range() {
  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());

  let mut available_in = 1usize;
  let mut input = usize::MAX as *const u8;
  let mut available_out = 0usize;
  let mut output = ptr::null_mut();
  let result = unsafe {
    BrotliDecoderDecompressStream(
      state,
      &mut available_in,
      &mut input,
      &mut available_out,
      &mut output,
      ptr::null_mut(),
    )
  };

  assert_eq!(
    result as i32,
    BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR as i32,
  );
  assert_eq!(
    unsafe { (*state).decompressor.error_code } as i32,
    BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS as i32,
  );
  unsafe { BrotliDecoderDestroyInstance(state) };
}

#[test]
fn stream_advances_the_original_output_pointer() {
  static ENCODED_FF_BYTES: &'static [u8] = b"\x1f\x07\x00\xf8\x27\xfe\x43\x84\x00\x00";

  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());

  let mut available_in = ENCODED_FF_BYTES.len();
  let mut input = ENCODED_FF_BYTES.as_ptr();
  let mut available_out = mem::size_of::<*mut u8>();
  let mut output = ptr::null_mut();
  let output_storage = &mut output as *mut *mut u8 as *mut u8;
  output = output_storage;

  // Decoding 0xff bytes overwrites `output` with usize::MAX. The cursor
  // must advance the original pointer instead of offsetting that value.
  let result = unsafe {
    BrotliDecoderDecompressStream(
      state,
      &mut available_in,
      &mut input,
      &mut available_out,
      &mut output,
      ptr::null_mut(),
    )
  };

  assert_ne!(
    result as i32,
    BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR as i32,
  );
  assert_eq!(available_out, 0);
  assert_eq!(
    output,
    output_storage.wrapping_add(mem::size_of::<*mut u8>()),
  );
  unsafe { BrotliDecoderDestroyInstance(state) };
}

#[test]
fn is_used_remains_true_after_byte_aligned_decode() {
  static ENCODED_FF_BYTES: &'static [u8] = b"\x1f\x07\x00\xf8\x27\xfe\x43\x84\x00\x00";

  let state = unsafe { BrotliDecoderCreateInstance(None, None, ptr::null_mut()) };
  assert!(!state.is_null());
  assert_eq!(unsafe { BrotliDecoderIsUsed(state) }, 0);

  let mut available_in = ENCODED_FF_BYTES.len();
  let mut input = ENCODED_FF_BYTES.as_ptr();
  let mut decoded = [0u8; 8];
  let mut available_out = decoded.len();
  let mut output = decoded.as_mut_ptr();
  let result = unsafe {
    BrotliDecoderDecompressStream(
      state,
      &mut available_in,
      &mut input,
      &mut available_out,
      &mut output,
      ptr::null_mut(),
    )
  };

  assert_eq!(
    result as i32,
    BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS as i32,
  );
  assert_eq!(decoded, [0xff; 8]);
  assert_eq!(unsafe { BrotliDecoderIsUsed(state) }, 1);
  unsafe { BrotliDecoderDestroyInstance(state) };
}
