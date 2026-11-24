extern crate brotli_decompressor;

use brotli_decompressor::{
  Allocator, BrotliDecoderIsFinished, BrotliDecompressStream, BrotliResult, BrotliState,
  HuffmanCode, SliceWrapper, SliceWrapperMut,
};
use std::marker::PhantomData;

// Supply our own allocator so these streaming tests also exercise the no_std
// library build, without enabling its optional std allocator dependency.
struct Memory<T>(Vec<T>);

impl<T> Default for Memory<T> {
  fn default() -> Self {
    Memory(Vec::new())
  }
}
impl<T> SliceWrapper<T> for Memory<T> {
  fn slice(&self) -> &[T] {
    &self.0
  }
}
impl<T> SliceWrapperMut<T> for Memory<T> {
  fn slice_mut(&mut self) -> &mut [T] {
    &mut self.0
  }
}

#[derive(Default)]
struct Heap<T>(PhantomData<T>);

impl<T: Clone + Default> Allocator<T> for Heap<T> {
  type AllocatedMemory = Memory<T>;
  fn alloc_cell(&mut self, size: usize) -> Memory<T> {
    Memory(vec![T::default(); size])
  }
  fn free_cell(&mut self, _memory: Memory<T>) {}
}

struct Decoder {
  state: BrotliState<Heap<u8>, Heap<u32>, Heap<HuffmanCode>>,
  output: Vec<u8>,
  output_limit: usize,
  calls: usize,
}

impl Decoder {
  fn new(output_limit: usize) -> Self {
    Decoder {
      state: BrotliState::new(Heap::default(), Heap::default(), Heap::default()),
      output: Vec::new(),
      output_limit,
      calls: 0,
    }
  }

  fn feed(
    &mut self,
    input: &[u8],
    output_chunk: usize,
    pause_output: bool,
  ) -> (BrotliResult, usize) {
    assert!(output_chunk != 0);
    let mut available_in = input.len();
    let mut input_offset = 0;
    let mut output = vec![0u8; output_chunk];
    loop {
      let out_len = if pause_output && self.calls % 5 == 0 {
        0
      } else {
        output.len()
      };
      let mut available_out = out_len;
      let mut output_offset = 0;
      let mut total_out = self.output.len();
      let result = BrotliDecompressStream(
        &mut available_in,
        &mut input_offset,
        input,
        &mut available_out,
        &mut output_offset,
        &mut output[..out_len],
        &mut total_out,
        &mut self.state,
      );
      self.calls += 1;
      assert_eq!(available_in + input_offset, input.len());
      assert_eq!(available_out + output_offset, out_len);
      self.output.extend_from_slice(&output[..output_offset]);
      // Together with the full-output assertion below, this bounds retries:
      // each non-paused NeedsMoreOutput call must produce bytes.
      assert!(
        self.output.len() <= self.output_limit,
        "decoded beyond the expected output"
      );
      assert_eq!(total_out, self.output.len());
      match result {
        BrotliResult::NeedsMoreOutput => assert_eq!(available_out, 0),
        BrotliResult::NeedsMoreInput => {
          assert_eq!(available_in, 0);
          return (result, input_offset);
        }
        _ => return (result, input_offset),
      }
    }
  }
}

fn fixtures() -> Vec<(&'static str, &'static [u8], &'static [u8])> {
  vec![
    (
      "empty",
      include_bytes!("../testdata/empty.compressed"),
      include_bytes!("../testdata/empty"),
    ),
    (
      "literal",
      include_bytes!("../testdata/x.compressed"),
      include_bytes!("../testdata/x"),
    ),
    (
      "copies",
      include_bytes!("../testdata/10x10y.compressed"),
      include_bytes!("../testdata/10x10y"),
    ),
    (
      "text",
      include_bytes!("../testdata/quickfox.compressed"),
      include_bytes!("../testdata/quickfox"),
    ),
    (
      "utf8",
      include_bytes!("../testdata/ukkonooa.compressed"),
      include_bytes!("../testdata/ukkonooa"),
    ),
    (
      "long distance",
      include_bytes!("../testdata/backward65536.compressed"),
      include_bytes!("../testdata/backward65536"),
    ),
    (
      "dictionary",
      include_bytes!("../testdata/alice29.txt.compressed"),
      include_bytes!("../testdata/alice29.txt"),
    ),
    (
      "mixed contexts",
      include_bytes!("../testdata/random_then_unicode.compressed"),
      include_bytes!("../testdata/random_then_unicode"),
    ),
    (
      "metablocks",
      include_bytes!("../testdata/metablock_reset.compressed"),
      include_bytes!("../testdata/metablock_reset"),
    ),
  ]
}

#[test]
fn streaming_matches_fixtures_at_refill_and_command_boundaries() {
  // The fast command path needs 28 input bytes. Also cross the 4- and 8-byte
  // load boundaries and keep input/output fragmentation independent.
  for (name, input, expected) in fixtures() {
    for &(input_chunk, output_chunk) in &[
      (1, 1),
      (4, 7),
      (7, 13),
      (8, 31),
      (27, 257),
      (28, 4096),
      (29, 17),
      (32, 33),
      (4096, 8192),
    ] {
      // Short fixtures cover byte-by-byte operation; large fixtures concentrate
      // on transitions into the fast command path and bulk input.
      if expected.len() > 1024 && input_chunk < 27 {
        continue;
      }
      let mut decoder = Decoder::new(expected.len());
      let mut consumed = 0;
      for chunk in input.chunks(input_chunk) {
        let (result, used) = decoder.feed(chunk, output_chunk, false);
        consumed += used;
        if consumed == input.len() {
          assert!(matches!(result, BrotliResult::ResultSuccess), "{}", name);
        } else {
          assert!(matches!(result, BrotliResult::NeedsMoreInput), "{}", name);
        }
      }
      assert_eq!(consumed, input.len(), "{}", name);
      assert!(
        decoder.output == expected,
        "{}: input {}, output {}",
        name,
        input_chunk,
        output_chunk
      );
      assert!(BrotliDecoderIsFinished(&decoder.state));
    }
  }
}

#[test]
fn every_short_stream_prefix_can_resume() {
  for (name, input, expected) in fixtures()
    .into_iter()
    .filter(|&(_, input, output)| input.len() <= 80 && output.len() <= 1024)
  {
    for split in 0..input.len() {
      let mut decoder = Decoder::new(expected.len());
      let (result, used) = decoder.feed(&input[..split], 3, false);
      assert!(
        matches!(result, BrotliResult::NeedsMoreInput),
        "{}: split {}",
        name,
        split
      );
      assert_eq!(used, split);
      assert!(
        expected.starts_with(&decoder.output),
        "{}: split {}",
        name,
        split
      );
      assert!(!BrotliDecoderIsFinished(&decoder.state));
      let (result, used) = decoder.feed(&input[split..], 7, false);
      assert!(
        matches!(result, BrotliResult::ResultSuccess),
        "{}: split {}",
        name,
        split
      );
      assert_eq!(used, input.len() - split);
      assert!(decoder.output == expected, "{}: split {}", name, split);
    }
  }
}

#[test]
fn output_backpressure_preserves_trailing_bytes() {
  const TRAILER: &[u8] = b"trailing data belongs to the caller";
  for (name, input, expected) in fixtures() {
    let mut storage = vec![0x55; 7]; // Also start the input at an unaligned address.
    storage.extend_from_slice(input);
    storage.extend_from_slice(TRAILER);
    for &output_chunk in &[1, 17, 4096] {
      if expected.len() > 1024 && output_chunk == 1 {
        continue;
      }
      let mut decoder = Decoder::new(expected.len());
      let (result, consumed) = decoder.feed(&storage[7..], output_chunk, true);
      assert!(matches!(result, BrotliResult::ResultSuccess), "{}", name);
      assert_eq!(consumed, input.len(), "{}", name);
      assert_eq!(&storage[7 + consumed..], TRAILER);
      assert!(decoder.output == expected, "{}", name);
      assert!(BrotliDecoderIsFinished(&decoder.state));
    }
  }
}

#[test]
fn truncated_fast_path_tails_resume_with_one_byte_chunks() {
  for (name, input, expected) in fixtures()
    .into_iter()
    .filter(|&(_, input, _)| input.len() > 80)
  {
    for tail in 1..33 {
      let split = input.len() - tail;
      let mut decoder = Decoder::new(expected.len());
      let (result, used) = decoder.feed(&input[..split], 4096, true);
      assert!(
        matches!(result, BrotliResult::NeedsMoreInput),
        "{}: tail {}",
        name,
        tail
      );
      assert_eq!(used, split);
      for (index, byte) in input[split..].chunks(1).enumerate() {
        let (result, used) = decoder.feed(byte, 17, true);
        assert_eq!(used, 1);
        if index + 1 == tail {
          assert!(
            matches!(result, BrotliResult::ResultSuccess),
            "{}: tail {}",
            name,
            tail
          );
        } else {
          assert!(
            matches!(result, BrotliResult::NeedsMoreInput),
            "{}: tail {}",
            name,
            tail
          );
        }
      }
      assert!(decoder.output == expected, "{}: tail {}", name, tail);
    }
  }
}

fn empty_dictionary_commands() -> Vec<u8> {
  // Build the compressed input shared by the two tests below: 16,777,217
  // commands that produce nothing, followed by one literal A. The details
  // below describe how that input is encoded.
  //
  // WBITS=16; a final compressed metablock with MLEN=1. Each category has
  // one block type, with no block lengths or switching tables in the stream.
  // NPOSTFIX=NDIRECT=0, LSB literal contexts, one literal and distance tree.
  // Simple Huffman trees: literal {65}, command {130, 138}, distance {42}.
  // Command 130 inserts nothing and copies a dictionary word of length 4.
  // Command 138 inserts one literal; that completes the metablock.
  let header = [0x02, 0x00, 0x00, 0x00, 0x44, 0x50, 0x09, 0xa2, 0x48, 0xa8];

  // Distance symbol 42 has 14 extra bits. Value 2052 gives distance 34817,
  // selecting dictionary word 0 with transform 34 (omit first 4), so the
  // command produces no output. Its command code is one zero bit and its
  // distance Huffman code takes zero bits: 15 bits per empty command.
  let command = 2052u16 << 1;
  let mut eight_commands = 0u128;
  for i in 0..8 {
    eight_commands |= (command as u128) << (15 * i);
  }
  let bytes = eight_commands.to_le_bytes();
  let mut input = Vec::with_capacity(header.len() + 15 * (1 << 21) + 2);
  input.extend_from_slice(&header);
  // More than 2^24 commands and distances, followed by command 138 (one bit).
  // Repeat in groups of eight to avoid storing a large unpacked bit array.
  for _ in 0..(1 << 21) {
    input.extend_from_slice(&bytes[..15]);
  }
  input.extend_from_slice(&(command | (1 << 15)).to_le_bytes());
  input
}

#[test]
fn empty_dictionary_commands_do_not_require_block_switches() {
  // Decode 16,777,217 commands that produce nothing, followed by one literal A.
  // There is only one block type, so no block switch is needed. The decoder
  // must reach the A instead of exhausting its 2^24 countdown and failing.
  let input = empty_dictionary_commands();
  let mut decoder = Decoder::new(1);
  let (result, used) = decoder.feed(&input, 1, false);
  assert!(matches!(result, BrotliResult::ResultSuccess));
  assert_eq!(used, input.len());
  assert_eq!(decoder.output, b"A");
  assert!(BrotliDecoderIsFinished(&decoder.state));
}

#[test]
fn stream_empty_dictionary_commands() {
  // The same check, with input arriving in pieces.
  let input = empty_dictionary_commands();
  for &chunk_size in &[4093, 65536] {
    let mut decoder = Decoder::new(1);
    // Parse the header and the first distance with one-byte input pieces.
    for byte in input[..12].chunks(1) {
      let (result, used) = decoder.feed(byte, 1, false);
      assert!(matches!(result, BrotliResult::NeedsMoreInput));
      assert_eq!(used, byte.len());
    }
    let mut consumed = 12;
    for chunk in input[12..].chunks(chunk_size) {
      let (result, used) = decoder.feed(chunk, 1, false);
      assert_eq!(used, chunk.len());
      consumed += used;
      if consumed == input.len() {
        assert!(matches!(result, BrotliResult::ResultSuccess));
      } else {
        assert!(matches!(result, BrotliResult::NeedsMoreInput));
      }
    }
    assert_eq!(decoder.output, b"A");
    assert!(BrotliDecoderIsFinished(&decoder.state));
  }
}
