#![cfg(feature = "ffi-api")]

// The C API's one-shot decoders with parameters,
// BrotliDecoderDecompressWithParameters and
// BrotliDecoderDecompressPreallocWithParameters. Without std the first one
// decodes through the no_std brotli_decode, so these run under both.

extern crate brotli_decompressor;

use brotli_decompressor::ffi::interface::BrotliDecoderResult;
use brotli_decompressor::ffi::{
  BrotliDecoderDecompress, BrotliDecoderDecompressPreallocWithParameters,
  BrotliDecoderDecompressWithParameters,
};
use brotli_decompressor::{
  BrotliDecoderErrorCode, BrotliDecoderParameter, BrotliDecoderReturnInfo, BrotliResult,
  HuffmanCode,
};

// TEXT compressed by the reference encoder as a Large Window Brotli stream
// (brotli --large_window=30 -q 11) and as an RFC 7932 stream (brotli -q 11).
const TEXT: &'static [u8] = b"Large Window Brotli! Large Window Brotli! Large Window Brotli!\n";
const LARGE_WINDOW_STREAM: [u8; 44] = [
  0x11, 0x5e, 0xf8, 0x00, 0xe0, 0x77, 0x26, 0xd8, 0xb1, 0x90, 0xc9, 0x29,
  0xc1, 0x4e, 0x78, 0x2d, 0xc4, 0xf4, 0x26, 0xa7, 0x14, 0x40, 0x2a, 0x84,
  0xae, 0xd2, 0xd8, 0x7a, 0x10, 0x42, 0x14, 0xf1, 0x47, 0x92, 0x40, 0x1e,
  0x00, 0x21, 0xbd, 0xff, 0x5f, 0xb8, 0xa9, 0x00,
];
const STANDARD_STREAM: [u8; 42] = [
  0xa1, 0xf0, 0x01, 0xc0, 0xef, 0x4c, 0xb0, 0x63, 0x21, 0x93, 0x53, 0x82,
  0x9d, 0xf0, 0x5a, 0x88, 0xe9, 0x4d, 0x4e, 0x29, 0x80, 0x54, 0x08, 0x5d,
  0xa5, 0xb1, 0xf5, 0x20, 0x84, 0x28, 0xe2, 0x8f, 0x24, 0x41, 0x1e, 0x80,
  0x48, 0xef, 0xff, 0x17, 0x6e, 0x2a,
];

const LARGE_WINDOW: BrotliDecoderParameter =
  BrotliDecoderParameter::BROTLI_DECODER_PARAM_LARGE_WINDOW;
const DISABLE_RING_BUFFER_REALLOCATION: BrotliDecoderParameter =
  BrotliDecoderParameter::BROTLI_DECODER_PARAM_DISABLE_RING_BUFFER_REALLOCATION;

// Output space for each one-shot decode. Without std, BrotliDecoderDecompress
// and BrotliDecoderDecompressWithParameters also take their u8 allocations
// from it.
const OUTPUT_SIZE: usize = 1 << 16;

// Runs `test` with room for the u32 and HuffmanCode pools, about 1.4 MiB, that
// the no_std brotli_decode keeps on the stack.
fn on_big_stack<F: FnOnce() + Send + 'static>(test: F) {
  std::thread::Builder::new().stack_size(16 << 20).spawn(test).unwrap().join().unwrap();
}

fn outcome(ret: BrotliDecoderReturnInfo, output: &[u8]) -> Result<Vec<u8>, i32> {
  match ret.result {
    BrotliResult::ResultSuccess => Ok(output[..ret.decoded_size].to_vec()),
    _ => Err(ret.error_code as i32),
  }
}

// Decodes `input` with both one-shot decoders with parameters, which must
// agree, into the decoded bytes or the error code.
unsafe fn decode_raw(
  input: &[u8],
  num_params: usize,
  param_keys: *const BrotliDecoderParameter,
  param_values: *const u32,
) -> Result<Vec<u8>, i32> {
  let mut output = vec![0u8; OUTPUT_SIZE];
  let ret = BrotliDecoderDecompressWithParameters(
    num_params, param_keys, param_values,
    input.len(), input.as_ptr(),
    output.len(), output.as_mut_ptr(),
  );
  let mut prealloc_output = vec![0u8; OUTPUT_SIZE];
  let mut scratch_u8 = vec![0u8; 1 << 17];
  let mut scratch_u32 = vec![0u32; 1 << 14];
  let mut scratch_hc = vec![HuffmanCode::default(); 1 << 16];
  let prealloc_ret = BrotliDecoderDecompressPreallocWithParameters(
    num_params, param_keys, param_values,
    input.len(), input.as_ptr(),
    prealloc_output.len(), prealloc_output.as_mut_ptr(),
    scratch_u8.len(), scratch_u8.as_mut_ptr(),
    scratch_u32.len(), scratch_u32.as_mut_ptr(),
    scratch_hc.len(), scratch_hc.as_mut_ptr(),
  );
  let result = outcome(ret, &output);
  assert_eq!(outcome(prealloc_ret, &prealloc_output), result);
  result
}

fn decode(input: &[u8], keys: &[BrotliDecoderParameter], values: &[u32]) -> Result<Vec<u8>, i32> {
  assert_eq!(keys.len(), values.len());
  unsafe { decode_raw(input, keys.len(), keys.as_ptr(), values.as_ptr()) }
}

fn error(code: BrotliDecoderErrorCode) -> Result<Vec<u8>, i32> {
  Err(code as i32)
}

#[test]
fn large_window_streams_can_be_rejected() {
  on_big_stack(|| {
    let text = Ok(TEXT.to_vec());
    let window_bits_error = error(BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_WINDOW_BITS);

    // Without parameters, large windows are accepted, as by
    // BrotliDecoderDecompress, and the arrays may be null.
    let mut output = vec![0u8; OUTPUT_SIZE];
    let mut decoded_size = output.len();
    let result = unsafe {
      BrotliDecoderDecompress(LARGE_WINDOW_STREAM.len(), LARGE_WINDOW_STREAM.as_ptr(),
                              &mut decoded_size, output.as_mut_ptr())
    };
    assert_eq!(result as i32, BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS as i32);
    assert_eq!(Ok(output[..decoded_size].to_vec()), text);
    assert_eq!(decode(&LARGE_WINDOW_STREAM, &[], &[]), text);
    assert_eq!(unsafe {
      decode_raw(&LARGE_WINDOW_STREAM, 0, std::ptr::null(), std::ptr::null())
    }, text);

    // Turning them off rejects them, as the reference C library's one-shot
    // decoder does, but still decodes RFC 7932 streams.
    assert_eq!(decode(&LARGE_WINDOW_STREAM, &[LARGE_WINDOW], &[0]), window_bits_error);
    assert_eq!(decode(&STANDARD_STREAM, &[LARGE_WINDOW], &[0]), text);

    // A later value replaces an earlier one, as when BrotliDecoderSetParameter
    // is called again.
    assert_eq!(decode(&LARGE_WINDOW_STREAM, &[LARGE_WINDOW; 2], &[0, 1]), text);
    assert_eq!(decode(&LARGE_WINDOW_STREAM, &[LARGE_WINDOW; 2], &[1, 0]), window_bits_error);

    // The other parameter is accepted too, alone or with this one.
    assert_eq!(decode(&STANDARD_STREAM, &[DISABLE_RING_BUFFER_REALLOCATION], &[1]), text);
    assert_eq!(decode(&LARGE_WINDOW_STREAM, &[DISABLE_RING_BUFFER_REALLOCATION, LARGE_WINDOW],
                      &[1, 0]),
               window_bits_error);
  });
}

#[test]
fn unknown_parameters_and_invalid_arrays_fail_the_call() {
  on_big_stack(|| {
    let invalid_arguments = error(BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS);
    // All zero, so still a valid key and value from any offset: only their
    // alignment can fail the misaligned arrays below.
    let keys = [DISABLE_RING_BUFFER_REALLOCATION; 2];
    let values = [0u32; 2];
    unsafe {
      assert_eq!(decode_raw(&STANDARD_STREAM, 1, keys.as_ptr(), values.as_ptr()),
                 Ok(TEXT.to_vec()));
      assert_eq!(decode_raw(&STANDARD_STREAM, 1, std::ptr::null(), values.as_ptr()),
                 invalid_arguments);
      assert_eq!(decode_raw(&STANDARD_STREAM, 1, keys.as_ptr(), std::ptr::null()),
                 invalid_arguments);
      let misaligned_keys = (keys.as_ptr() as *const u8).add(1) as *const BrotliDecoderParameter;
      assert_eq!(decode_raw(&STANDARD_STREAM, 1, misaligned_keys, values.as_ptr()),
                 invalid_arguments);
      let misaligned_values = (values.as_ptr() as *const u8).add(1) as *const u32;
      assert_eq!(decode_raw(&STANDARD_STREAM, 1, keys.as_ptr(), misaligned_values),
                 invalid_arguments);
    }

    // Values that name no parameter, stored as a C caller would, in an
    // int-sized enum. They fail the whole call, rather than be skipped, even
    // after a known parameter. 256 and 1 << 24 would pass for one if read as
    // too few bytes, or in the wrong byte order.
    assert_eq!(std::mem::size_of::<BrotliDecoderParameter>(), std::mem::size_of::<u32>());
    for &key in &[2u32, 256, 1 << 24, u32::MAX] {
      let c_keys = [LARGE_WINDOW as u32, key];
      assert_eq!(unsafe {
        decode_raw(&STANDARD_STREAM, 2, c_keys.as_ptr() as *const BrotliDecoderParameter,
                   values.as_ptr())
      }, invalid_arguments, "key {}", key);
    }
  });
}
