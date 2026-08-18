#![cfg(test)]
#![cfg(all(feature = "ffi-api", feature = "std"))]

use std::mem;
use std::ptr;

use brotli_decompressor::ffi::interface::BrotliDecoderResult;
use brotli_decompressor::ffi::{
  BrotliDecoderCreateInstance, BrotliDecoderDecompressStream, BrotliDecoderDestroyInstance,
  BrotliDecoderErrorCode, BrotliDecoderIsUsed,
};

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
