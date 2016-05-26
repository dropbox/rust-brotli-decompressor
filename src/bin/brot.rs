mod integration_tests;
extern crate brotli_no_stdlib as brotli;
extern crate core;


#[macro_use]
extern crate alloc_no_stdlib;
mod heap_alloc;
use heap_alloc::HeapAllocator;
use core::ops;
use alloc_no_stdlib::{Allocator, SliceWrapperMut, SliceWrapper,
            StackAllocator, AllocatedStackMemory, bzero};

//use alloc::{SliceWrapper,SliceWrapperMut, StackAllocator, AllocatedStackMemory, Allocator};
use brotli::{BrotliDecompressStream, BrotliState, BrotliResult, HuffmanCode};
pub use brotli::FILE_BUFFER_SIZE;
use std::io::{self, Read, Write, ErrorKind, Error};
use std::time::Duration;
use std::env;

use std::fs::File;

use std::path::Path;

#[cfg(not(feature="disable-timer"))]
use std::time::SystemTime;

#[cfg(feature="disable-timer")]
fn now() -> Duration {
    return Duration::new(0, 0);
}
#[cfg(not(feature="disable-timer"))]
fn now() -> SystemTime {
    return SystemTime::now();
}

#[cfg(not(feature="disable-timer"))]
fn elapsed(start : SystemTime) -> (Duration, bool) {
    match start.elapsed() {
        Ok(delta) => return (delta, false),
        _ => return (Duration::new(0, 0), true),
    }
}

#[cfg(feature="disable-timer")]
fn elapsed(_start : Duration) -> (Duration, bool) {
    return (Duration::new(0, 0), true);
}

declare_stack_allocator_struct!(MemPool, 4096, calloc);


fn _write_all<OutputType> (w : &mut OutputType, buf : &[u8]) -> Result<(), io::Error>
where OutputType: Write {
    let mut total_written : usize = 0;
    while total_written < buf.len() {
        match w.write(&buf[total_written..]) {
            Err(e) => {
                match e.kind() {
                    ErrorKind::Interrupted => continue,
                    _ => return Err(e),
                }
            },
            Ok(cur_written) => {
                if cur_written == 0 {
                     return Err(Error::new(ErrorKind::UnexpectedEof, "Write EOF"));
                }
                total_written += cur_written;
            }
        }
    }
    Ok(())
}

//trace_macros!(true);

pub fn decompress<InputType, OutputType> (r : &mut InputType, mut w : &mut OutputType) -> Result<(), io::Error>
where InputType: Read, OutputType: Write {
    return decompress_internal(r, w, 4096 * 1024, 4096 * 1024);
}
static mut ibuffer : [u8;4096 * 1024] = [0; 4096 * 1024];
static mut obuffer : [u8;4096 * 1024] = [0; 4096 * 1024];
pub fn decompress_internal<InputType, OutputType> (r : &mut InputType, mut w : &mut OutputType, input_buffer_limit : usize, output_buffer_limit : usize) -> Result<(), io::Error>
where InputType: Read, OutputType: Write {
  let mut u8_alloc = HeapAllocator::<u8>{default_value : 0};
  let mut u32_alloc = HeapAllocator::<u32>{default_value : 0};
  let mut hc_alloc = HeapAllocator::<HuffmanCode>{default_value : HuffmanCode::default()};
  //test(calloc_u8_allocator);
  let mut brotli_state = BrotliState::new(u8_alloc, u32_alloc, hc_alloc);
  let mut input = unsafe{&mut ibuffer[0..input_buffer_limit]};
  let mut output = unsafe{&mut obuffer[0..output_buffer_limit]};
  let mut available_out : usize = output.len();

  //let amount = try!(r.read(&mut buf));
  let mut available_in : usize = 0;
  let mut input_offset : usize = 0;
  let mut output_offset : usize = 0;
  let mut result : BrotliResult = BrotliResult::NeedsMoreInput;
  let mut total = Duration::new(0, 0);
  let mut timing_error : bool = false;
  loop {
      match result {
          BrotliResult::NeedsMoreInput => {
              input_offset = 0;
              match r.read(&mut input) {
                  Err(e) => {
                      match e.kind() {
                          ErrorKind::Interrupted => continue,
                          _ => return Err(e),
                      }
                  },
                  Ok(size) => {
                      if size == 0 {
                          return Err(Error::new(ErrorKind::UnexpectedEof, "Read EOF"));
                      }
                      available_in = size;
                  },
              }
          },
          BrotliResult::NeedsMoreOutput => {
              try!(_write_all(&mut w, &output[..output_offset]));
              output_offset = 0;
          },
          BrotliResult::ResultSuccess => break,
          BrotliResult::ResultFailure => panic!("FAILURE"),
      }
      let mut written :usize = 0;
      let start = now();
      result = BrotliDecompressStream(&mut available_in, &mut input_offset, &input,
                                      &mut available_out, &mut output_offset, &mut output,
                                      &mut written, &mut brotli_state);

      let (delta, err) = elapsed(start);
      if err {
          timing_error = true;
      }
      total = total + delta;
      if output_offset != 0 {
          try!(_write_all(&mut w, &output[..output_offset]));
          output_offset = 0;
          available_out = output.len()
      }
  }
  if timing_error {
      let _r = writeln!(&mut std::io::stderr(), "Timing error\n");
  } else {
      let _r = writeln!(&mut std::io::stderr(), "Time {:}.{:09}\n",
                        total.as_secs(),
                        total.subsec_nanos());
  }
  brotli_state.BrotliStateCleanup();
  Ok(())
}

fn main() {
    if env::args_os().len() > 1 {
        let mut first = true;
        for argument in env::args() {
            if first {
               first = false;
               continue;
            }
            let mut input = match File::open(&Path::new(&argument)) {
                Err(why) => panic!("couldn't open {}: {:?}", argument,
                                                       why),
                Ok(file) => file,
            };
            let oa = argument + ".original";
            let mut output = match File::create(&Path::new(&oa), ) {
                Err(why) => panic!("couldn't open file for writing: {:} {:?}", oa, why),
                Ok(file) => file,
            };
            match decompress(&mut input, &mut output) {
                Ok(_) => {},
                Err(e) => panic!("Error {:?}", e),
            }
            drop(output);
            drop(input);
        }
    } else {
        match decompress(&mut io::stdin(), &mut io::stdout()) {
            Ok(_) => return,
            Err(e) => panic!("Error {:?}", e),
        }
    }
}
