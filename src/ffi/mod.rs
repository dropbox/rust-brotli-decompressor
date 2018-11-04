#![cfg(not(feature="safe"))]

#[no_mangle]
#[cfg(feature="std")]
use std::{thread,panic, io};
#[cfg(feature="std")]
use std::io::Write;
use core;
use core::slice;
pub mod interface;
pub mod alloc_util;
use self::alloc_util::SubclassableAllocator;
use alloc::Allocator;
use self::interface::{CAllocator, c_void, BrotliDecoderParameter, BrotliDecoderResult, brotli_alloc_func, brotli_free_func};
use ::BrotliResult;
pub use super::state::BrotliDecoderErrorCode;
#[repr(C)]
#[no_mangle]
pub struct BrotliDecoderState {
    pub custom_allocator: CAllocator,
    pub decompressor: ::BrotliState<SubclassableAllocator,
                                    SubclassableAllocator,
                                    SubclassableAllocator>,
}

#[cfg(not(feature="std"))]
fn brotli_new_decompressor_without_custom_alloc(_to_box: BrotliDecoderState) -> *mut BrotliDecoderState{
    panic!("Must supply allocators if calling divans when compiled without features=std");
}

#[cfg(feature="std")]
fn brotli_new_decompressor_without_custom_alloc(to_box: BrotliDecoderState) -> *mut BrotliDecoderState{
    alloc_util::Box::<BrotliDecoderState>::into_raw(
        alloc_util::Box::<BrotliDecoderState>::new(to_box))
}


#[no_mangle]
pub unsafe extern fn BrotliDecoderCreateInstance(
    alloc_func: brotli_alloc_func,
    free_func: brotli_free_func,
    opaque: *mut c_void,
) -> *mut BrotliDecoderState {
    match catch_panic_state(|| {
      let allocators = CAllocator {
        alloc_func:alloc_func,
        free_func:free_func,
        opaque:opaque,
      };
      let custom_dictionary = <SubclassableAllocator as Allocator<u8>>::AllocatedMemory::default();
      let to_box = BrotliDecoderState {
        custom_allocator: allocators.clone(),
        decompressor: ::BrotliState::new_with_custom_dictionary(
          SubclassableAllocator::new(allocators.clone()),
          SubclassableAllocator::new(allocators.clone()),
          SubclassableAllocator::new(allocators.clone()),
          custom_dictionary,
        ),
      };
      if let Some(alloc) = alloc_func {
        if free_func.is_none() {
            panic!("either both alloc and free must exist or neither");
        }
        let ptr = alloc(allocators.opaque, core::mem::size_of::<BrotliDecoderState>());
        let brotli_decoder_state_ptr = core::mem::transmute::<*mut c_void, *mut BrotliDecoderState>(ptr);
        core::ptr::write(brotli_decoder_state_ptr, to_box);
        brotli_decoder_state_ptr
      } else {
        brotli_new_decompressor_without_custom_alloc(to_box)
      }
    }) {
        Ok(ret) => ret,
        Err(e) => {
            error_print(e);
            core::ptr::null_mut()
        },
    }
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderSetParameter(_state_ptr: *mut BrotliDecoderState,
                                       _selector: BrotliDecoderParameter,
                                       _value: u32) {
  // not implemented
}

#[cfg(not(feature="std"))] // error always since no default allocator
#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompress(
  _encoded_size: usize,
  _encoded_buffer: *const u8,
  _decoded_size: *mut usize,
  _decoded_buffer: *mut u8,
) -> BrotliDecoderResult {
  BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR // no allocator
}

#[cfg(feature="std")] // this requires a default allocator
#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompress(
  encoded_size: usize,
  encoded_buffer: *const u8,
  decoded_size: *mut usize,
  decoded_buffer: *mut u8) -> BrotliDecoderResult {
  let mut total_out = 0;
  let mut available_in = encoded_size;
  let mut next_in = encoded_buffer;
  let mut available_out = *decoded_size;
  let mut next_out = decoded_buffer;
  let s = BrotliDecoderCreateInstance(
    None,
    None,
    core::ptr::null_mut(),
  );
  if s.is_null() { // if the allocation failed
      return BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR;
  }
  let result = BrotliDecoderDecompressStream(
    s, &mut available_in, &mut next_in, &mut available_out, &mut next_out, &mut total_out);
  *decoded_size = total_out;
  BrotliDecoderDestroyInstance(s);
  if let BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS = result {
    BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS
  } else {
    BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR
  }
}

#[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
fn catch_panic<F:FnOnce()->BrotliDecoderResult+panic::UnwindSafe>(f: F) -> thread::Result<BrotliDecoderResult> {
    panic::catch_unwind(f)
}

#[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
fn catch_panic_state<F:FnOnce()->*mut BrotliDecoderState+panic::UnwindSafe>(f: F) -> thread::Result<*mut BrotliDecoderState> {
    panic::catch_unwind(f)
}

#[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
fn error_print<Err:core::fmt::Debug>(err: Err) {
    let _ign = writeln!(&mut io::stderr(), "Internal Error {:?}", err);
}

// can't catch panics in a reliable way without std:: configure with panic=abort. These shouldn't happen
#[cfg(any(not(feature="std"), feature="pass-through-ffi-panics"))]
fn catch_panic<F:FnOnce()->BrotliDecoderResult>(f: F) -> Result<BrotliDecoderResult, ()> {
    Ok(f())
}

#[cfg(any(not(feature="std"), feature="pass-through-ffi-panics"))]
fn catch_panic_state<F:FnOnce()->*mut BrotliDecoderState>(f: F) -> Result<*mut BrotliDecoderState, ()> {
    Ok(f())
}

#[cfg(any(not(feature="std"), feature="pass-through-ffi-panics"))]
fn error_print<Err>(_err: Err) {
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompressStream(
    state_ptr: *mut BrotliDecoderState,
    available_in: *mut usize,
    input_buf_ptr: *mut*const u8,
    available_out: *mut usize,
    output_buf_ptr: *mut*mut u8,
    mut total_out: *mut usize) -> BrotliDecoderResult {
    match catch_panic(move || {
    let mut input_offset = 0usize;
    let mut output_offset = 0usize;
    let mut fallback_total_out = 0usize;
    if total_out.is_null() {
        total_out = &mut fallback_total_out;
    }
    let result;
    {
        let input_buf = slice::from_raw_parts(*input_buf_ptr, *available_in);
        let output_buf = slice::from_raw_parts_mut(*output_buf_ptr, *available_out);
            result = match super::decode::BrotliDecompressStream(
                &mut *available_in,
                &mut input_offset,
                input_buf,
                &mut *available_out,
                &mut output_offset,
                output_buf,
                &mut *total_out,
                &mut (*state_ptr).decompressor,
            ) {
                BrotliResult::ResultSuccess => BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS,
                BrotliResult::ResultFailure => BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR,
                BrotliResult::NeedsMoreInput => BrotliDecoderResult::BROTLI_DECODER_RESULT_NEEDS_MORE_INPUT ,
                BrotliResult::NeedsMoreOutput => BrotliDecoderResult::BROTLI_DECODER_RESULT_NEEDS_MORE_OUTPUT ,
            };
    }
    *input_buf_ptr = (*input_buf_ptr).offset(input_offset as isize);
    *output_buf_ptr = (*output_buf_ptr).offset(output_offset as isize);
                                           result
    }) {
        Ok(ret) => ret,
        Err(readable_err) => { // if we panic (completely unexpected) then we should report it back to C and print
            error_print(readable_err);
            (*state_ptr).decompressor.error_code = BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE;
            BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR
        }
    }
}

#[cfg(feature="std")]
unsafe fn free_decompressor_no_custom_alloc(state_ptr: *mut BrotliDecoderState) {
    let _state = alloc_util::Box::from_raw(state_ptr);
}

#[cfg(not(feature="std"))]
unsafe fn free_decompressor_no_custom_alloc(_state_ptr: *mut BrotliDecoderState) {
    unreachable!();
}


#[no_mangle]
pub unsafe extern fn BrotliDecoderMallocU8(state_ptr: *mut BrotliDecoderState, size: usize) -> *mut u8 {
    if let Some(alloc_fn) = (*state_ptr).custom_allocator.alloc_func {
        return core::mem::transmute::<*mut c_void, *mut u8>(alloc_fn((*state_ptr).custom_allocator.opaque, size));
    } else {
        return alloc_util::alloc_stdlib(size);
    }
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderFreeU8(state_ptr: *mut BrotliDecoderState, data: *mut u8, size: usize) {
    if let Some(free_fn) = (*state_ptr).custom_allocator.free_func {
        free_fn((*state_ptr).custom_allocator.opaque, core::mem::transmute::<*mut u8, *mut c_void>(data));
    } else {
        alloc_util::free_stdlib(data, size);
    }
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderMallocUsize(state_ptr: *mut BrotliDecoderState, size: usize) -> *mut usize {
    if let Some(alloc_fn) = (*state_ptr).custom_allocator.alloc_func {
        return core::mem::transmute::<*mut c_void, *mut usize>(alloc_fn((*state_ptr).custom_allocator.opaque,
                                                                         size * core::mem::size_of::<usize>()));
    } else {
        return alloc_util::alloc_stdlib(size);
    }
}
#[no_mangle]
pub unsafe extern fn BrotliDecoderFreeUsize(state_ptr: *mut BrotliDecoderState, data: *mut usize, size: usize) {
    if let Some(free_fn) = (*state_ptr).custom_allocator.free_func {
        free_fn((*state_ptr).custom_allocator.opaque, core::mem::transmute::<*mut usize, *mut c_void>(data));
    } else {
        alloc_util::free_stdlib(data, size);
    }
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderDestroyInstance(state_ptr: *mut BrotliDecoderState) {
    if let Some(_) = (*state_ptr).custom_allocator.alloc_func {
        if let Some(free_fn) = (*state_ptr).custom_allocator.free_func {
            let _to_free = core::ptr::read(state_ptr);
            let ptr = core::mem::transmute::<*mut BrotliDecoderState, *mut c_void>(state_ptr);
            free_fn((*state_ptr).custom_allocator.opaque, ptr);
        }
    } else {
        free_decompressor_no_custom_alloc(state_ptr);
    }
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderHasMoreOutput(state_ptr: *const BrotliDecoderState) -> i32 {
  if super::decode::BrotliDecoderHasMoreOutput(&(*state_ptr).decompressor) {1} else {0}
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderTakeOutput(state_ptr: *mut BrotliDecoderState, size: *mut usize) -> *const u8 {
  super::decode::BrotliDecoderTakeOutput(&mut (*state_ptr).decompressor, &mut *size).as_ptr()
}



#[no_mangle]
pub unsafe extern fn BrotliDecoderIsUsed(state_ptr: *const BrotliDecoderState) -> i32 {
  if super::decode::BrotliDecoderIsUsed(&(*state_ptr).decompressor) {1} else {0}
}
#[no_mangle]
pub unsafe extern fn BrotliDecoderIsFinished(state_ptr: *const BrotliDecoderState) -> i32 {
  if super::decode::BrotliDecoderIsFinished(&(*state_ptr).decompressor) {1} else {0}
}
#[no_mangle]
pub unsafe extern fn BrotliDecoderGetErrorCode(state_ptr: *const BrotliDecoderState) -> BrotliDecoderErrorCode {
  super::decode::BrotliDecoderGetErrorCode(&(*state_ptr).decompressor)
}

#[no_mangle]
pub extern fn BrotliDecoderErrorString(c: BrotliDecoderErrorCode) -> *const u8 {
  match c {
  BrotliDecoderErrorCode::BROTLI_DECODER_NO_ERROR => "NO_ERROR\0",
  /* Same as BrotliDecoderResult values */
  BrotliDecoderErrorCode::BROTLI_DECODER_SUCCESS => "SUCCESS\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_NEEDS_MORE_INPUT => "NEEDS_MORE_INPUT\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_NEEDS_MORE_OUTPUT => "NEEDS_MORE_OUTPUT\0",

  /* Errors caused by invalid input */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_EXUBERANT_NIBBLE => "ERROR_FORMAT_EXUBERANT_NIBBLE\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_RESERVED => "ERROR_FORMAT_RESERVED\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_EXUBERANT_META_NIBBLE => "ERROR_FORMAT_EXUBERANT_META_NIBBLE\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_SIMPLE_HUFFMAN_ALPHABET => "ERROR_FORMAT_SIMPLE_HUFFMAN_ALPHABET\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_SIMPLE_HUFFMAN_SAME => "ERROR_FORMAT_SIMPLE_HUFFMAN_SAME\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_CL_SPACE => "ERROR_FORMAT_FL_SPACE\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_HUFFMAN_SPACE => "ERROR_FORMAT_HUFFMAN_SPACE\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_CONTEXT_MAP_REPEAT => "ERROR_FORMAT_CONTEXT_MAP_REPEAT\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_BLOCK_LENGTH_1 =>"ERROR_FORMAT_BLOCK_LENGTH_1\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_BLOCK_LENGTH_2 =>"ERROR_FORMAT_BLOCK_LENGTH_2\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_TRANSFORM => "ERROR_FORMAT_TRANSFORM\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_DICTIONARY =>"ERROR_FORMAT_DICTIONARY\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_WINDOW_BITS =>"ERROR_FORMAT_WINDOW_BITS\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_PADDING_1 =>"ERROR_FORMAT_PADDING_1\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_PADDING_2 =>"ERROR_FORMAT_PADDING_2\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_DISTANCE =>"ERROR_FORMAT_DISTANCE\0",

  /* -17..-18 codes are reserved */

  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_DICTIONARY_NOT_SET => "ERROR_DICTIONARY_NOT_SET\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS => "ERROR_INVALID_ARGUMENTS\0",

  /* Memory allocation problems */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MODES => "ERROR_ALLOC_CONTEXT_MODES\0",
  /* Literal => insert and distance trees together */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_TREE_GROUPS => "ERROR_ALLOC_TREE_GROUPS\0",
  /* -23..-24 codes are reserved for distinct tree groups */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MAP => "ERROR_ALLOC_CONTEXT_MAP\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_1 => "ERROR_ALLOC_RING_BUFFER_1\0",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_2 => "ERROR_ALLOC_RING_BUFFER_2\0",
  /* -28..-29 codes are reserved for dynamic ring-buffer allocation */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_BLOCK_TYPE_TREES => "ERROR_ALLOC_BLOCK_TYPE_TREES\0",

  /* "Impossible" states */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE => "ERROR_UNREACHABLE\0",
  }.as_ptr()
}

#[no_mangle]
pub extern fn BrotliDecoderVersion() -> u32 {
  0x1000f00
}
