#![cfg(not(feature="safe"))]

#[no_mangle]
use core;
use core::slice;
pub mod interface;
pub mod alloc_util;
use self::alloc_util::SubclassableAllocator;
use alloc::Allocator;
use self::interface::{CAllocator, c_void, BrotliDecoderParameter, BrotliDecoderResult, brotli_alloc_func, brotli_free_func};
use ::BrotliResult;
use super::state::BrotliDecoderErrorCode;
#[repr(C)]
#[no_mangle]
pub struct BrotliDecoderState {
    pub custom_allocator: CAllocator,
    pub decompressor: ::BrotliState<SubclassableAllocator,
                                    SubclassableAllocator,
                                    SubclassableAllocator>,
}

#[cfg(feature="no-stdlib")]
fn brotli_new_decompressor_without_custom_alloc(_to_box: BrotliDecoderState) -> *mut BrotliDecoderState{
    panic!("Must supply allocators if calling divans when compiled with features=no-stdlib");
}

#[cfg(not(feature="no-stdlib"))]
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
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderSetParameter(_state_ptr: *mut BrotliDecoderState,
                                       _selector: BrotliDecoderParameter,
                                       _value: u32) {
  // not implemented
}

#[cfg(feature="no-stdlib")] // error always since no default allocator
#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompress(
  _encoded_size: usize,
  _encoded_buffer: *const u8,
  _decoded_size: *mut usize,
  _decoded_buffer: *mut u8,
) -> BrotliDecoderResult {
  BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR // no allocator
}

#[cfg(not(feature="no-stdlib"))] // this requires a default allocator
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

#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompressStream(
    state_ptr: *mut BrotliDecoderState,
    available_in: *mut usize,
    input_buf_ptr: *mut*const u8,
    available_out: *mut usize,
    output_buf_ptr: *mut*mut u8,
    total_out: *mut usize) -> BrotliDecoderResult {
    let mut input_offset = 0usize;
    let mut output_offset = 0usize;
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
    return result;
}

#[cfg(not(feature="no-stdlib"))]
unsafe fn free_decompressor_no_custom_alloc(state_ptr: *mut BrotliDecoderState) {
    let _state = alloc_util::Box::from_raw(state_ptr);
}

#[cfg(feature="no-stdlib")]
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
  BrotliDecoderErrorCode::BROTLI_DECODER_NO_ERROR => "NO_ERROR",
  /* Same as BrotliDecoderResult values */
  BrotliDecoderErrorCode::BROTLI_DECODER_SUCCESS => "SUCCESS",
  BrotliDecoderErrorCode::BROTLI_DECODER_NEEDS_MORE_INPUT => "NEEDS_MORE_INPUT",
  BrotliDecoderErrorCode::BROTLI_DECODER_NEEDS_MORE_OUTPUT => "NEEDS_MORE_OUTPUT",

  /* Errors caused by invalid input */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_EXUBERANT_NIBBLE => "ERROR_FORMAT_EXUBERANT_NIBBLE",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_RESERVED => "ERROR_FORMAT_RESERVED",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_EXUBERANT_META_NIBBLE => "ERROR_FORMAT_EXUBERANT_META_NIBBLE",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_SIMPLE_HUFFMAN_ALPHABET => "ERROR_FORMAT_SIMPLE_HUFFMAN_ALPHABET",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_SIMPLE_HUFFMAN_SAME => "ERROR_FORMAT_SIMPLE_HUFFMAN_SAME",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_CL_SPACE => "ERROR_FORMAT_FL_SPACE",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_HUFFMAN_SPACE => "ERROR_FORMAT_HUFFMAN_SPACE",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_CONTEXT_MAP_REPEAT => "ERROR_FORMAT_CONTEXT_MAP_REPEAT",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_BLOCK_LENGTH_1 =>"ERROR_FORMAT_BLOCK_LENGTH_1",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_BLOCK_LENGTH_2 =>"ERROR_FORMAT_BLOCK_LENGTH_2",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_TRANSFORM => "ERROR_FORMAT_TRANSFORM",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_DICTIONARY =>"ERROR_FORMAT_DICTIONARY",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_WINDOW_BITS =>"ERROR_FORMAT_WINDOW_BITS",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_PADDING_1 =>"ERROR_FORMAT_PADDING_1",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_PADDING_2 =>"ERROR_FORMAT_PADDING_2",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_FORMAT_DISTANCE =>"ERROR_FORMAT_DISTANCE",

  /* -17..-18 codes are reserved */

  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_DICTIONARY_NOT_SET => "ERROR_DICTIONARY_NOT_SET",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS => "ERROR_INVALID_ARGUMENTS",

  /* Memory allocation problems */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MODES => "ERROR_ALLOC_CONTEXT_MODES",
  /* Literal => insert and distance trees together */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_TREE_GROUPS => "ERROR_ALLOC_TREE_GROUPS",
  /* -23..-24 codes are reserved for distinct tree groups */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MAP => "ERROR_ALLOC_CONTEXT_MAP",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_1 => "ERROR_ALLOC_RING_BUFFER_1",
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_2 => "ERROR_ALLOC_RING_BUFFER_2",
  /* -28..-29 codes are reserved for dynamic ring-buffer allocation */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_ALLOC_BLOCK_TYPE_TREES => "ERROR_ALLOC_BLOCK_TYPE_TREES",

  /* "Impossible" states */
  BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE => "ERROR_UNREACHABLE",
  }.as_ptr()
}

#[no_mangle]
pub extern fn BrotliDecoderVersion() -> u32 {
  0x1000f00
}
