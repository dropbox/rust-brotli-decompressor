#![no_std]
#![allow(non_snake_case)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

#[macro_use]
// <-- for debugging, remove xprintln from bit_reader and replace with println
#[cfg(not(feature="no-stdlib"))]
extern crate std;
#[cfg(not(feature="no-stdlib"))]
use std::io::{self, Error, ErrorKind, Read, Write};

#[macro_use]
extern crate alloc_no_stdlib as alloc;
pub use alloc::{AllocatedStackMemory, Allocator, SliceWrapper, SliceWrapperMut, StackAllocator};

#[cfg(not(feature="no-stdlib"))]
pub use alloc::HeapAlloc;
#[cfg(all(feature="unsafe",not(feature="no-stdlib")))]
pub use alloc::HeapAllocUninitialized;
#[macro_use]
mod memory;
pub mod dictionary;
mod brotli_alloc;
#[macro_use]
mod bit_reader;
mod huffman;
mod state;
mod prefix;
mod context;
pub mod transform;
mod test;
mod decode;
pub mod io_wrappers;
pub mod reader;
pub mod writer;
pub use huffman::{HuffmanCode, HuffmanTreeGroup};
pub use state::BrotliState;
pub mod ffi;
pub use reader::{DecompressorCustomIo};

#[cfg(not(feature="no-stdlib"))]
pub use reader::{Decompressor};

pub use writer::{DecompressorWriterCustomIo};
#[cfg(not(feature="no-stdlib"))]
pub use writer::{DecompressorWriter};

// use io_wrappers::write_all;
pub use io_wrappers::{CustomRead, CustomWrite};
#[cfg(not(feature="no-stdlib"))]
pub use io_wrappers::{IntoIoReader, IoReaderWrapper, IntoIoWriter, IoWriterWrapper};

// interface
// pub fn BrotliDecompressStream(mut available_in: &mut usize,
//                               input_offset: &mut usize,
//                               input: &[u8],
//                               mut available_out: &mut usize,
//                               mut output_offset: &mut usize,
//                               mut output: &mut [u8],
//                               mut total_out: &mut usize,
//                               mut s: &mut BrotliState<AllocU8, AllocU32, AllocHC>);

pub use decode::{BrotliDecompressStream, BrotliResult};




#[cfg(not(any(feature="unsafe", feature="no-stdlib")))]
pub fn BrotliDecompress<InputType, OutputType>(r: &mut InputType,
                                               w: &mut OutputType)
                                               -> Result<(), io::Error>
  where InputType: Read,
        OutputType: Write
{
  let mut input_buffer: [u8; 4096] = [0; 4096];
  let mut output_buffer: [u8; 4096] = [0; 4096];
  BrotliDecompressCustomAlloc(r,
                              w,
                              &mut input_buffer[..],
                              &mut output_buffer[..],
                              HeapAlloc::<u8> { default_value: 0 },
                              HeapAlloc::<u32> { default_value: 0 },
                              HeapAlloc::<HuffmanCode> {
                                default_value: HuffmanCode::default(),
                              })
}

#[cfg(not(feature="no-stdlib"))]
pub fn BrotliDecompressCustomDict<InputType, OutputType>(r: &mut InputType,
                                                         w: &mut OutputType,
                                                         input_buffer:&mut [u8],
                                                         output_buffer:&mut [u8],
                                                         custom_dictionary:std::vec::Vec<u8>)
                                                          -> Result<(), io::Error>
  where InputType: Read,
        OutputType: Write
{
  let mut alloc_u8 = brotli_alloc::BrotliAlloc::<u8>::new();
  let mut input_buffer_backing;
  let mut output_buffer_backing;
  {
  let mut borrowed_input_buffer = input_buffer;
  let mut borrowed_output_buffer = output_buffer;
  if borrowed_input_buffer.len() == 0 {
     input_buffer_backing = alloc_u8.alloc_cell(4096);
     borrowed_input_buffer = input_buffer_backing.slice_mut();
  }
  if borrowed_output_buffer.len() == 0 {
     output_buffer_backing = alloc_u8.alloc_cell(4096);
     borrowed_output_buffer = output_buffer_backing.slice_mut();
  }
  let dict = alloc_u8.take_ownership(custom_dictionary);
  BrotliDecompressCustomIoCustomDict(&mut IoReaderWrapper::<InputType>(r),
                              &mut IoWriterWrapper::<OutputType>(w),
                              borrowed_input_buffer,
                              borrowed_output_buffer,
                              alloc_u8,
                              brotli_alloc::BrotliAlloc::<u32>::new(),
                              brotli_alloc::BrotliAlloc::<HuffmanCode>::new(),
                              dict,
                              Error::new(ErrorKind::UnexpectedEof, "Unexpected EOF"))
  }
}

#[cfg(all(feature="unsafe",not(feature="no-stdlib")))]
pub fn BrotliDecompress<InputType, OutputType>(r: &mut InputType,
                                               w: &mut OutputType)
                                               -> Result<(), io::Error>
  where InputType: Read,
        OutputType: Write
{
  let mut input_buffer: [u8; 4096] = [0; 4096];
  let mut output_buffer: [u8; 4096] = [0; 4096];
  BrotliDecompressCustomAlloc(r,
                              w,
                              &mut input_buffer[..],
                              &mut output_buffer[..],
                              unsafe { HeapAllocUninitialized::<u8>::new() },
                              unsafe { HeapAllocUninitialized::<u32>::new() },
                              unsafe { HeapAllocUninitialized::<HuffmanCode>::new() })
}


#[cfg(not(feature="no-stdlib"))]
pub fn BrotliDecompressCustomAlloc<InputType,
                                   OutputType,
                                   AllocU8: Allocator<u8>,
                                   AllocU32: Allocator<u32>,
                                   AllocHC: Allocator<HuffmanCode>>
  (r: &mut InputType,
   w: &mut OutputType,
   input_buffer: &mut [u8],
   output_buffer: &mut [u8],
   alloc_u8: AllocU8,
   alloc_u32: AllocU32,
   alloc_hc: AllocHC)
   -> Result<(), io::Error>
  where InputType: Read,
        OutputType: Write
{
  BrotliDecompressCustomIo(&mut IoReaderWrapper::<InputType>(r),
                           &mut IoWriterWrapper::<OutputType>(w),
                           input_buffer,
                           output_buffer,
                           alloc_u8,
                           alloc_u32,
                           alloc_hc,
                           Error::new(ErrorKind::UnexpectedEof, "Unexpected EOF"))
}
pub fn BrotliDecompressCustomIo<ErrType,
                                InputType,
                                OutputType,
                                AllocU8: Allocator<u8>,
                                AllocU32: Allocator<u32>,
                                AllocHC: Allocator<HuffmanCode>>
  (r: &mut InputType,
   w: &mut OutputType,
   input_buffer: &mut [u8],
   output_buffer: &mut [u8],
   alloc_u8: AllocU8,
   alloc_u32: AllocU32,
   alloc_hc: AllocHC,
   unexpected_eof_error_constant: ErrType)
   -> Result<(), ErrType>
  where InputType: CustomRead<ErrType>,
        OutputType: CustomWrite<ErrType>
{
  BrotliDecompressCustomIoCustomDict(r, w, input_buffer, output_buffer, alloc_u8, alloc_u32, alloc_hc, AllocU8::AllocatedMemory::default(), unexpected_eof_error_constant)
}
pub fn BrotliDecompressCustomIoCustomDict<ErrType,
                                InputType,
                                OutputType,
                                AllocU8: Allocator<u8>,
                                AllocU32: Allocator<u32>,
                                AllocHC: Allocator<HuffmanCode>>
  (r: &mut InputType,
   w: &mut OutputType,
   input_buffer: &mut [u8],
   output_buffer: &mut [u8],
   alloc_u8: AllocU8,
   alloc_u32: AllocU32,
   alloc_hc: AllocHC,
   custom_dictionary: AllocU8::AllocatedMemory,
   unexpected_eof_error_constant: ErrType)
   -> Result<(), ErrType>
  where InputType: CustomRead<ErrType>,
        OutputType: CustomWrite<ErrType>
{
  let mut brotli_state = BrotliState::new_with_custom_dictionary(alloc_u8, alloc_u32, alloc_hc, custom_dictionary);
  assert!(input_buffer.len() != 0);
  assert!(output_buffer.len() != 0);
  let mut available_out: usize = output_buffer.len();

  let mut available_in: usize = 0;
  let mut input_offset: usize = 0;
  let mut output_offset: usize = 0;
  let mut result: BrotliResult = BrotliResult::NeedsMoreInput;
  loop {
    match result {
      BrotliResult::NeedsMoreInput => {
        input_offset = 0;
        match r.read(input_buffer) {
          Err(e) => return Err(e),
          Ok(size) => {
            if size == 0 {
              return Err(unexpected_eof_error_constant);
            }
            available_in = size;
          }
        }
      }
      BrotliResult::NeedsMoreOutput => {
        let mut total_written: usize = 0;
        while total_written < output_offset {
          // this would be a call to write_all
          match w.write(&output_buffer[total_written..output_offset]) {
            Err(e) => return Result::Err(e),
            Ok(cur_written) => {
              assert_eq!(cur_written == 0, false); // not allowed by the contract
              total_written += cur_written;
            }
          }
        }

        output_offset = 0;
      }
      BrotliResult::ResultSuccess => break,
      BrotliResult::ResultFailure => return Err(unexpected_eof_error_constant),
    }
    let mut written: usize = 0;
    result = BrotliDecompressStream(&mut available_in,
                                    &mut input_offset,
                                    input_buffer,
                                    &mut available_out,
                                    &mut output_offset,
                                    output_buffer,
                                    &mut written,
                                    &mut brotli_state);

    if output_offset != 0 {
      let mut total_written: usize = 0;
      while total_written < output_offset {
        match w.write(&output_buffer[total_written..output_offset]) {
          Err(e) => return Result::Err(e),
          // CustomResult::Transient(e) => continue,
          Ok(cur_written) => {
            assert_eq!(cur_written == 0, false); // not allowed by the contract
            total_written += cur_written;
          }
        }
      }
      output_offset = 0;
      available_out = output_buffer.len()
    }
  }
  brotli_state.BrotliStateCleanup();
  Ok(())
}


#[cfg(not(feature="no-stdlib"))]
pub fn copy_from_to<R: io::Read, W: io::Write>(mut r: R, mut w: W) -> io::Result<usize> {
  let mut buffer: [u8; 65536] = [0; 65536];
  let mut out_size: usize = 0;
  loop {
    match r.read(&mut buffer[..]) {
      Err(e) => {
        if let io::ErrorKind::Interrupted =  e.kind() {
          continue
        }
        return Err(e);
      }
      Ok(size) => {
        if size == 0 {
          break;
        } else {
          match w.write_all(&buffer[..size]) {
            Err(e) => {
              if let io::ErrorKind::Interrupted = e.kind() {
                continue
              }
              return Err(e);
            }
            Ok(_) => out_size += size,
          }
        }
      }
    }
  }
  Ok(out_size)
}
