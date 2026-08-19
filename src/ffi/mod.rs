#[cfg(feature="std")]
use std::{thread,panic, io, boxed, any, string};
#[cfg(feature="std")]
use std::io::Write;
use core;
use core::slice;
use core::ops;
pub mod interface;
pub mod alloc_util;
use self::alloc_util::SubclassableAllocator;
use alloc::{Allocator, SliceWrapper, SliceWrapperMut, StackAllocator, AllocatedStackMemory, bzero};
use self::interface::{CAllocator, c_void, BrotliDecoderParameter, BrotliDecoderResult, brotli_alloc_func, brotli_free_func};
use ::BrotliResult;
use ::BrotliDecoderReturnInfo;
use ::brotli_decode;
pub use ::HuffmanCode;
pub use super::state::{BrotliDecoderErrorCode, BrotliState};

pub unsafe fn slice_from_raw_parts_or_nil<'a, T>(data: *const T, len: usize) -> &'a [T] {
    if len == 0 {
        return &[];
    }
    slice::from_raw_parts(data, len)
}

pub unsafe fn slice_from_raw_parts_or_nil_mut<'a, T>(data: *mut T, len: usize) -> &'a mut [T] {
    if len == 0 {
        return &mut [];
    }
    slice::from_raw_parts_mut(data, len)
}

trait MaxSliceLen {
    const MAX_SLICE_LEN: usize;
}

impl<T> MaxSliceLen for T {
    const MAX_SLICE_LEN: usize = if core::mem::size_of::<T>() == 0 {
        usize::MAX
    } else {
        (isize::MAX as usize) / core::mem::size_of::<T>()
    };
}

// Rejects the pointer/length pairs that would make `slice::from_raw_parts` trip a
// non-unwinding "unsafe precondition" panic not catchable by `catch_unwind`.
fn is_valid_slice_ptr<T>(data: *const T, len: usize) -> bool {
    if len == 0 {
        return true;
    }
    if data.is_null() {
        return false;
    }
    if (data as usize) % core::mem::align_of::<T>() != 0 {
        return false;
    }
    if len > T::MAX_SLICE_LEN {
        return false;
    }
    (data as usize).checked_add(len * core::mem::size_of::<T>()).is_some()
}

unsafe fn checked_slice_from_raw_parts_or_nil<'a, T>(
    data: *const T,
    len: usize,
) -> Option<&'a [T]> {
    if !is_valid_slice_ptr(data, len) {
        return None;
    }
    Some(slice_from_raw_parts_or_nil(data, len))
}

unsafe fn checked_slice_from_raw_parts_or_nil_mut<'a, T>(
    data: *mut T,
    len: usize,
) -> Option<&'a mut [T]> {
    if !is_valid_slice_ptr(data, len) {
        return None;
    }
    Some(slice_from_raw_parts_or_nil_mut(data, len))
}

#[cfg(feature="std")]
type BrotliAdditionalErrorData = boxed::Box<dyn any::Any + Send + 'static>;
#[cfg(not(feature="std"))]
type BrotliAdditionalErrorData = ();

#[repr(C)]
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
    match catch_panic(|| {
      let allocators = CAllocator {
        alloc_func:alloc_func,
        free_func:free_func,
        opaque:opaque,
      };
      let custom_dictionary = <SubclassableAllocator as Allocator<u8>>::AllocatedMemory::default();
      let mut decompressor = ::BrotliState::new_with_custom_dictionary(
        SubclassableAllocator::new(allocators.clone()),
        SubclassableAllocator::new(allocators.clone()),
        SubclassableAllocator::new(allocators.clone()),
        custom_dictionary,
      );
      if decompressor.context_map_table.slice().len() == 0 {
        return core::ptr::null_mut();
      }
      decompressor.large_window = false;
      let to_box = BrotliDecoderState {
        custom_allocator: allocators.clone(),
        decompressor: decompressor,
      };
      if let Some(alloc) = alloc_func {
        if free_func.is_none() {
            panic!("either both alloc and free must exist or neither");
        }
        let ptr = alloc(allocators.opaque, core::mem::size_of::<BrotliDecoderState>());
        if ptr.is_null() {
            return core::ptr::null_mut();
        }
        let brotli_decoder_state_ptr = core::mem::transmute::<*mut c_void, *mut BrotliDecoderState>(ptr);
        core::ptr::write(brotli_decoder_state_ptr, to_box);
        brotli_decoder_state_ptr
      } else {
        brotli_new_decompressor_without_custom_alloc(to_box)
      }
    }) {
        Ok(ret) => ret,
        Err(mut e) => {
            error_print(core::ptr::null_mut(), &mut e);
            core::ptr::null_mut()
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn BrotliDecoderSetParameter(state_ptr: *mut BrotliDecoderState,
                                             selector: BrotliDecoderParameter,
                                             value: u32) -> i32 {
  if state_ptr.is_null() {
    return 0;
  }
  let state = &mut (*state_ptr).decompressor;
  match &state.state {
    &super::state::BrotliRunningState::BROTLI_STATE_UNINITED => {},
    _ => return 0,
  }
  match selector {
    BrotliDecoderParameter::BROTLI_DECODER_PARAM_DISABLE_RING_BUFFER_REALLOCATION => {
      state.canny_ringbuffer_allocation = value == 0;
    },
    BrotliDecoderParameter::BROTLI_DECODER_PARAM_LARGE_WINDOW => {
      state.large_window = value != 0;
    },
  }
  1
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompressPrealloc(
  encoded_size: usize,
  encoded_buffer: *const u8,
  decoded_size: usize,
  decoded_buffer: *mut u8,
  scratch_u8_size: usize,
  scratch_u8_buffer: *mut u8,
  scratch_u32_size: usize,
  scratch_u32_buffer: *mut u32,
  scratch_hc_size: usize,
  scratch_hc_buffer: *mut HuffmanCode,
) -> BrotliDecoderReturnInfo {
  catch_panic_return_info(move || {
    let input = match checked_slice_from_raw_parts_or_nil(encoded_buffer, encoded_size) {
      Some(input) => input,
      None => return invalid_argument_return_info(),
    };
    let output = match checked_slice_from_raw_parts_or_nil_mut(decoded_buffer, decoded_size) {
      Some(output) => output,
      None => return invalid_argument_return_info(),
    };
    let scratch_u8 = match checked_slice_from_raw_parts_or_nil_mut(
      scratch_u8_buffer,
      scratch_u8_size,
    ) {
      Some(scratch_u8) => scratch_u8,
      None => return invalid_argument_return_info(),
    };
    let scratch_u32 = match checked_slice_from_raw_parts_or_nil_mut(
      scratch_u32_buffer,
      scratch_u32_size,
    ) {
      Some(scratch_u32) => scratch_u32,
      None => return invalid_argument_return_info(),
    };
    let scratch_hc = match checked_slice_from_raw_parts_or_nil_mut(
      scratch_hc_buffer,
      scratch_hc_size,
    ) {
      Some(scratch_hc) => scratch_hc,
      None => return invalid_argument_return_info(),
    };
    ::brotli_decode_prealloc(input, output, scratch_u8, scratch_u32, scratch_hc)
  })
}

unsafe fn brotli_decoder_decompress_with_return_info(
  encoded_size: usize,
  encoded_buffer: *const u8,
  decoded_size: usize,
  decoded_buffer: *mut u8,
) -> BrotliDecoderReturnInfo {
  let input = match checked_slice_from_raw_parts_or_nil(encoded_buffer, encoded_size) {
    Some(input) => input,
    None => return invalid_argument_return_info(),
  };
  let output_scratch = match checked_slice_from_raw_parts_or_nil_mut(
    decoded_buffer,
    decoded_size,
  ) {
    Some(output_scratch) => output_scratch,
    None => return invalid_argument_return_info(),
  };
  ::brotli_decode(input, output_scratch)
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompressWithReturnInfo(
  encoded_size: usize,
  encoded_buffer: *const u8,
  decoded_size: usize,
  decoded_buffer: *mut u8,
) -> BrotliDecoderReturnInfo {
  catch_panic_return_info(move || {
    brotli_decoder_decompress_with_return_info(
      encoded_size,
      encoded_buffer,
      decoded_size,
      decoded_buffer,
    )
  })
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompress(
  encoded_size: usize,
  encoded_buffer: *const u8,
  decoded_size: *mut usize,
  decoded_buffer: *mut u8,
) -> BrotliDecoderResult {
  if !is_valid_slice_ptr(decoded_size as *const usize, 1) {
    return BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR;
  }
  match catch_panic(move || {
    let res = brotli_decoder_decompress_with_return_info(
      encoded_size,
      encoded_buffer,
      *decoded_size,
      decoded_buffer,
    );
    *decoded_size = res.decoded_size;
    match res.result {
        BrotliResult::ResultSuccess => BrotliDecoderResult::BROTLI_DECODER_RESULT_SUCCESS,
        _ => BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR
    }
  }) {
      Ok(ret) => ret,
      Err(mut readable_err) => {
          error_print(core::ptr::null_mut(), &mut readable_err);
          *decoded_size = 0;
          BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR
      },
  }
}

#[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
fn catch_panic<T, F>(f: F) -> thread::Result<T>
where F: FnOnce() -> T + panic::UnwindSafe {
    panic::catch_unwind(f)
}

fn copy_error_string(src: &[u8]) -> [u8;256] {
    let mut dst = [0u8;256];
    let xlen = core::cmp::min(src.len(), dst.len() - 1);
    dst.split_at_mut(xlen).0.clone_from_slice(src.split_at(xlen).0);
    dst
}

fn invalid_argument_return_info() -> BrotliDecoderReturnInfo {
    let error_code = BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS;
    BrotliDecoderReturnInfo {
        decoded_size: 0,
        error_string: copy_error_string(::state::BrotliDecoderErrorStr(error_code).as_bytes()),
        error_code: error_code,
        result: BrotliResult::ResultFailure,
    }
}

#[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
fn panic_return_info(err: &BrotliAdditionalErrorData) -> BrotliDecoderReturnInfo {
    let error_string = if let Some(st) = err.downcast_ref::<&str>() {
        copy_error_string(st.as_bytes())
    } else if let Some(st) = err.downcast_ref::<string::String>() {
        copy_error_string(st.as_bytes())
    } else {
        copy_error_string(
          ::state::BrotliDecoderErrorStr(
            BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE,
          ).as_bytes(),
        )
    };
    BrotliDecoderReturnInfo {
        decoded_size: 0,
        error_string: error_string,
        error_code: BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE,
        result: BrotliResult::ResultFailure,
    }
}

#[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
fn catch_panic_return_info<F>(f: F) -> BrotliDecoderReturnInfo
where F: FnOnce() -> BrotliDecoderReturnInfo + panic::UnwindSafe {
    match catch_panic(f) {
        Ok(ret) => ret,
        Err(mut readable_err) => {
            let ret = panic_return_info(&readable_err);
            unsafe {
                error_print(core::ptr::null_mut(), &mut readable_err);
            }
            ret
        },
    }
}

#[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
unsafe fn error_print(state_ptr: *mut BrotliDecoderState, err: &mut BrotliAdditionalErrorData) {
    if let Some(st) = err.downcast_ref::<&str>() {
        if !state_ptr.is_null() {
          (*state_ptr).decompressor.mtf_or_error_string = Err(copy_error_string(st.as_bytes()));
        }
        let _ign = writeln!(&mut io::stderr(), "panic: {}", st);
    } else {
        if let Some(st) = err.downcast_ref::<string::String>() {
          if !state_ptr.is_null() {
            (*state_ptr).decompressor.mtf_or_error_string = Err(copy_error_string(st.as_bytes()));
          }
          let _ign = writeln!(&mut io::stderr(), "Internal Error {:?}", st);
        } else {
            let _ign = writeln!(&mut io::stderr(), "Internal Error {:?}", err);
        }
    }
}

// can't catch panics in a reliable way without std:: configure with panic=abort. These shouldn't happen
#[cfg(any(not(feature="std"), feature="pass-through-ffi-panics"))]
fn catch_panic<T, F>(f: F) -> Result<T, BrotliAdditionalErrorData>
where F: FnOnce() -> T {
    Ok(f())
}

#[cfg(any(not(feature="std"), feature="pass-through-ffi-panics"))]
fn catch_panic_return_info<F>(f: F) -> BrotliDecoderReturnInfo
where F: FnOnce() -> BrotliDecoderReturnInfo {
    f()
}

#[cfg(any(not(feature="std"), feature="pass-through-ffi-panics"))]
fn error_print(_state_ptr: *mut BrotliDecoderState, _err: &mut BrotliAdditionalErrorData) {
}

#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompressStream(
    state_ptr: *mut BrotliDecoderState,
    available_in: *mut usize,
    input_buf_ptr: *mut*const u8,
    available_out: *mut usize,
    output_buf_ptr: *mut*mut u8,
    mut total_out: *mut usize) -> BrotliDecoderResult {
    if state_ptr.is_null() ||
       available_in.is_null() ||
       input_buf_ptr.is_null() ||
       available_out.is_null() ||
       output_buf_ptr.is_null() {
        if !state_ptr.is_null() {
            (*state_ptr).decompressor.error_code =
                BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS;
        }
        return BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR;
    }
    match catch_panic(move || {
    let mut input_offset = 0usize;
    let mut output_offset = 0usize;
    let mut fallback_total_out = 0usize;
    if total_out.is_null() {
        total_out = &mut fallback_total_out;
    }
    let result: BrotliDecoderResult;
    let input_ptr = *input_buf_ptr;
    let output_ptr = *output_buf_ptr;
    {
        let input_buf = match checked_slice_from_raw_parts_or_nil(
            input_ptr,
            *available_in,
        ) {
            Some(input_buf) => input_buf,
            None => {
                (*state_ptr).decompressor.error_code =
                    BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS;
                return BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR;
            },
        };
        let output_buf = match checked_slice_from_raw_parts_or_nil_mut(
            output_ptr,
            *available_out,
        ) {
            Some(output_buf) => output_buf,
            None => {
                (*state_ptr).decompressor.error_code =
                    BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS;
                return BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR;
            },
        };
            result = super::decode::BrotliDecompressStream(
                &mut *available_in,
                &mut input_offset,
                input_buf,
                &mut *available_out,
                &mut output_offset,
                output_buf,
                &mut *total_out,
                &mut (*state_ptr).decompressor,
            ).into();
    }
    *input_buf_ptr = input_ptr.offset(input_offset as isize);
    *output_buf_ptr = output_ptr.offset(output_offset as isize);
                                           result
    }) {
        Ok(ret) => ret,
        Err(mut readable_err) => { // if we panic (completely unexpected) then we should report it back to C and print
            error_print(state_ptr, &mut readable_err);
            (*state_ptr).decompressor.error_code = BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE;
            BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR
        }
    }
}

/// Equivalent to BrotliDecoderDecompressStream but with no optional arg and no double indirect ptrs
#[no_mangle]
pub unsafe extern fn BrotliDecoderDecompressStreaming(
    state_ptr: *mut BrotliDecoderState,
    available_in: *mut usize,
    mut input_buf_ptr: *const u8,
    available_out: *mut usize,
    mut output_buf_ptr: *mut u8) -> BrotliDecoderResult {
    BrotliDecoderDecompressStream(state_ptr,
                                  available_in,
                                  &mut input_buf_ptr,
                                  available_out,
                                  &mut output_buf_ptr,
                                  core::ptr::null_mut())
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
        let alloc_size = match size.checked_mul(core::mem::size_of::<usize>()) {
            Some(alloc_size) => alloc_size,
            None => return core::ptr::null_mut(),
        };
        return core::mem::transmute::<*mut c_void, *mut usize>(alloc_fn((*state_ptr).custom_allocator.opaque,
                                                                         alloc_size));
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

// Attaches a dictionary to the decoder, like the C API of the same name.
// The data is copied, so unlike the C API it need not outlive the decoder.
// Must be called before any input is processed.
// Returns 1 on success, 0 on failure.
#[no_mangle]
pub unsafe extern "C" fn BrotliDecoderAttachDictionary(
    state_ptr: *mut BrotliDecoderState,
    dict_type: i32,
    data_size: usize,
    data: *const u8,
) -> i32 {
  if state_ptr.is_null() {
    return 0;
  }
  let is_serialized = match dict_type {
    0 => false,
    1 => true,
    _ => return 0,
  };
  let data_slice = match checked_slice_from_raw_parts_or_nil(data, data_size) {
    Some(data_slice) => data_slice,
    None => return 0,
  };
  match catch_panic(move || {
    match (*state_ptr).decompressor.state {
      super::state::BrotliRunningState::BROTLI_STATE_UNINITED => {},
      _ => return 0,
    }
    let dict = {
      let alloc_u8 = &mut (*state_ptr).decompressor.alloc_u8;
      let mut dict = alloc_u8.alloc_cell(data_size);
      if dict.slice().len() != data_size {
        alloc_u8.free_cell(dict);
        return 0;
      }
      dict.slice_mut().clone_from_slice(data_slice);
      dict
    };
    let ok = if is_serialized {
        (*state_ptr).decompressor.attach_serialized_dictionary(dict)
    } else {
        (*state_ptr).decompressor.attach_dictionary(dict)
    };
    if ok {1} else {0}
  }) {
    Ok(ret) => ret,
    Err(mut readable_err) => {
      error_print(state_ptr, &mut readable_err);
      0
    },
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
pub unsafe extern fn BrotliDecoderGetErrorString(state_ptr: *const BrotliDecoderState) -> *const u8 {
  if !state_ptr.is_null() {
    if let &Err(ref msg) = &(*state_ptr).decompressor.mtf_or_error_string {
      // important: this must be a ref
      // so stack memory is not returned
      return msg.as_ptr();
    }
  }
  BrotliDecoderErrorString(super::decode::BrotliDecoderGetErrorCode(&(*state_ptr).decompressor))
}
#[no_mangle]
pub extern fn BrotliDecoderErrorString(c: BrotliDecoderErrorCode) -> *const u8 {
    ::state::BrotliDecoderErrorStr(c).as_ptr()
}


#[no_mangle]
pub extern fn BrotliDecoderVersion() -> u32 {
  0x1000f00
}

#[cfg(test)]
mod tests {
  use super::*;

  fn assert_invalid_argument(ret: BrotliDecoderReturnInfo) {
    assert_eq!(ret.decoded_size, 0);
    assert_eq!(
      ret.error_code as i32,
      BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS as i32,
    );
    let expected = ::state::BrotliDecoderErrorStr(
      BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_INVALID_ARGUMENTS,
    ).as_bytes();
    assert_eq!(&ret.error_string[..expected.len()], expected);
    match ret.result {
      BrotliResult::ResultFailure => {},
      _ => panic!("expected invalid arguments to return failure"),
    }
  }

  #[test]
  fn one_shot_rejects_null_input_buffer() {
    let ret = unsafe {
      BrotliDecoderDecompressWithReturnInfo(
        1,
        core::ptr::null(),
        0,
        core::ptr::null_mut(),
      )
    };

    assert_invalid_argument(ret);
  }

  #[test]
  fn prealloc_rejects_misaligned_scratch_buffer() {
    let mut scratch_u32 = [0u32; 2];
    let misaligned_scratch_u32 =
      unsafe { (scratch_u32.as_mut_ptr() as *mut u8).add(1) as *mut u32 };
    let ret = unsafe {
      BrotliDecoderDecompressPrealloc(
        0,
        core::ptr::null(),
        0,
        core::ptr::null_mut(),
        0,
        core::ptr::null_mut(),
        1,
        misaligned_scratch_u32,
        0,
        core::ptr::null_mut(),
      )
    };

    assert_invalid_argument(ret);
  }

  #[test]
  fn one_shot_rejects_null_decoded_size() {
    let ret = unsafe {
      BrotliDecoderDecompress(
        0,
        core::ptr::null(),
        core::ptr::null_mut(),
        core::ptr::null_mut(),
      )
    };

    assert_eq!(
      ret as i32,
      BrotliDecoderResult::BROTLI_DECODER_RESULT_ERROR as i32,
    );
  }

  #[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
  #[test]
  fn one_shot_panic_returns_error_info() {
    let ret = catch_panic_return_info(|| -> BrotliDecoderReturnInfo {
      panic!("ffi one-shot panic");
    });

    assert_eq!(ret.decoded_size, 0);
    assert_eq!(
      ret.error_code as i32,
      BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE as i32,
    );
    assert_eq!(&ret.error_string[..18], b"ffi one-shot panic");
    assert_eq!(ret.error_string[18], 0);
    match ret.result {
      BrotliResult::ResultFailure => {},
      _ => panic!("expected one-shot panic to return failure"),
    }
  }

  #[cfg(all(feature="std", not(feature="pass-through-ffi-panics")))]
  #[test]
  fn prealloc_catches_scratch_exhaustion() {
    let ret = unsafe {
      BrotliDecoderDecompressPrealloc(
        0,
        core::ptr::null(),
        0,
        core::ptr::null_mut(),
        0,
        core::ptr::null_mut(),
        0,
        core::ptr::null_mut(),
        0,
        core::ptr::null_mut(),
      )
    };

    assert_eq!(ret.decoded_size, 0);
    assert_eq!(
      ret.error_code as i32,
      BrotliDecoderErrorCode::BROTLI_DECODER_ERROR_UNREACHABLE as i32,
    );
    match ret.result {
      BrotliResult::ResultFailure => {},
      _ => panic!("expected scratch exhaustion to return failure"),
    }
  }

  #[test]
  fn set_parameter() {
    let set_parameter: unsafe extern "C" fn(
      *mut BrotliDecoderState,
      BrotliDecoderParameter,
      u32,
    ) -> i32 = BrotliDecoderSetParameter;

    unsafe {
      let state = BrotliDecoderCreateInstance(None, None, core::ptr::null_mut());
      assert!(!state.is_null());
      assert!(!(*state).decompressor.large_window);
      assert!((*state).decompressor.canny_ringbuffer_allocation);

      assert_eq!(set_parameter(
        state,
        BrotliDecoderParameter::BROTLI_DECODER_PARAM_DISABLE_RING_BUFFER_REALLOCATION,
        1,
      ), 1);
      assert!(!(*state).decompressor.canny_ringbuffer_allocation);

      assert_eq!(set_parameter(
        state,
        BrotliDecoderParameter::BROTLI_DECODER_PARAM_LARGE_WINDOW,
        1,
      ), 1);
      assert!((*state).decompressor.large_window);

      (*state).decompressor.state =
        super::super::state::BrotliRunningState::BROTLI_STATE_INITIALIZE;
      assert_eq!(set_parameter(
        state,
        BrotliDecoderParameter::BROTLI_DECODER_PARAM_LARGE_WINDOW,
        0,
      ), 0);
      assert!((*state).decompressor.large_window);

      BrotliDecoderDestroyInstance(state);
    }
  }
}
