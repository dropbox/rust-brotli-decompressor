use super::{HuffmanCode, HuffmanTreeGroup};
use super::huffman::histogram::{ANSTable, HistogramSpec};
use super::BrotliResult;
use super::bit_reader;
use core::ops::AddAssign;
use alloc;
use alloc::Allocator;
use alloc::SliceWrapper;
use alloc::SliceWrapperMut;

pub trait BoolTrait {
  const VALUE: bool;
}
pub struct TrueBoolTrait {}
impl BoolTrait for TrueBoolTrait {
  const VALUE: bool = true;
}
pub struct FalseBoolTrait{}
impl BoolTrait for FalseBoolTrait {
  const VALUE: bool = false;
}

pub type Speculative = TrueBoolTrait;
pub type Unconditional = FalseBoolTrait;

pub trait SymbolCast {
  fn cast(data: u16) -> Self;
}

impl SymbolCast for u8 {
  fn cast(data:u16) -> Self {
    return data as u8
  }
}
impl SymbolCast for u16 {
  fn cast(data:u16) -> Self {
    return data
  }
}


pub trait EntropyEncoder {
    fn put<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocU32:Allocator<u32>,AllocHC:Allocator<HuffmanCode>, Spec:HistogramSpec>(&mut self, group:HuffmanTreeGroup<AllocU32, AllocHC>, prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, prior: u8, symbol: Symbol, output:&mut [u8], output_offset:&mut usize) -> BrotliResult;
  fn put_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, symbol: Symbol, output: &mut[u8], output_offset:&mut usize) -> BrotliResult;
  fn put_uniform(&mut self, nbits: u8, symbol: u16, output: &mut [u8], output_offset: &mut usize);
  fn flush(&mut self, output: &mut[u8], output_offset:&mut usize) -> BrotliResult;
}
use core::fmt;
pub trait EntropyDecoder {
  type SpeculativeState: fmt::Debug+PartialEq+Eq;
  fn set_active(&mut self);
  fn set_inactive(&mut self);
  fn bit_reader(&mut self) -> &mut bit_reader::BrotliBitReader;
  fn br(&self) -> &bit_reader::BrotliBitReader;
  fn warmup(&mut self, input:&[u8]) -> BrotliResult;
  fn begin_metablock(&mut self, input:&[u8]) -> BrotliResult;
  fn sufficient_bits(&mut self, nbits: u8) -> bool;
  fn placeholder(&self) -> Self::SpeculativeState;
  fn preload<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
             AllocS:Allocator<Symbol>,
             AllocH: Allocator<u32>,
             Spec:HistogramSpec>(&mut self,
                                   group:&[&[HuffmanCode];256],
                                   prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                   prior: u8,
                                   input:&[u8]) -> (u32, u32);
  // precondition: input has at least 4 bytes
  fn get_preloaded<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
                   AllocS:Allocator<Symbol>,
                   AllocH: Allocator<u32>,
                   Spec:HistogramSpec>(&mut self,
                                          group:&[&[HuffmanCode];256],
                                          prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                          prior: u8,
                                          preloaded: (u32, u32),
                                          input:&[u8]) -> Symbol;
  // precondition: input has at least 4 bytes
  fn get<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
         AllocS:Allocator<Symbol>,
         AllocH: Allocator<u32>,
         Spec:HistogramSpec,
         Speculative:BoolTrait>(&mut self,
                                group:&[&[HuffmanCode];256],
                                prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                prior: u8,
                                input:&[u8],
                                is_speculative: Speculative) -> (Symbol, BrotliResult);
  fn get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, l1numbits: u8, input:&[u8]) -> Symbol;
  // precondition: input has at least 4 bytes
    fn safe_get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, l1numbits: u8, input: &[u8], is_speculative: Speculative) -> (Symbol, BrotliResult);
    // precondition: input has at least 4 bytes
    fn get_uniform<Speculative:BoolTrait>(&mut self, nbits: u8, input: &[u8], is_speculative: Speculative) -> (u32, BrotliResult);
    fn begin_speculative(&mut self) -> Self::SpeculativeState;
    fn commit_speculative(&mut self);
    fn abort_speculative(&mut self, val:Self::SpeculativeState);
}


#[derive(Default)]
pub struct HuffmanDecoder {
  active: bool,
  br: bit_reader::BrotliBitReader,
}

impl EntropyDecoder  for HuffmanDecoder {
  fn set_active(&mut self) {
    self.active = true;
  }
  fn set_inactive(&mut self) {
    self.active = false;
  }
  fn bit_reader(&mut self) -> &mut bit_reader::BrotliBitReader {
    &mut self.br
  }
  fn br(&self) -> &bit_reader::BrotliBitReader {
    &self.br
  }
  fn warmup(&mut self, input:&[u8]) -> BrotliResult{
    if self.active {
      if (!bit_reader::BrotliWarmupBitReader(&mut self.br,
                                             input)) {
        return BrotliResult::NeedsMoreInput;
      }
    }
    BrotliResult::ResultSuccess
  }
  fn begin_metablock(&mut self, input:&[u8]) -> BrotliResult{
    // nothing to do for standard huffman-coded items
    BrotliResult::ResultSuccess
  }
  fn sufficient_bits(&mut self, nbits: u8) -> bool{
    if !self.active {
      return true;
    }
    bit_reader::BrotliCheckInputAmount(self.br(), nbits.into())
  }
    fn preload<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
               AllocS:Allocator<Symbol>,
               AllocH: Allocator<u32>,
               Spec:HistogramSpec>(&mut self,
                                   group:&[&[HuffmanCode];256],
                                   prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                   prior: u8,
                                   input:&[u8]) -> (u32, u32){
        (0,0)
    }
    // precondition: input has at least 4 bytes
  fn get_preloaded<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
         AllocS:Allocator<Symbol>,
         AllocH: Allocator<u32>,
         Spec:HistogramSpec>(&mut self,
                                group:&[&[HuffmanCode];256],
                                prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                prior: u8,
                                preloaded: (u32, u32),
                                input:&[u8]) -> Symbol {
    Symbol::from(0)
  }
  fn get<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
         AllocS:Allocator<Symbol>,
         AllocH: Allocator<u32>,
         Spec:HistogramSpec,
         Speculative:BoolTrait>(&mut self,
                                group:&[&[HuffmanCode];256],
                                prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                prior: u8,
                                input:&[u8],
                                is_speculative: Speculative) -> (Symbol, BrotliResult) {
    (Symbol::from(0), BrotliResult::ResultSuccess)
  }
  fn get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, table:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, l1numbits: u8, input: &[u8]) -> Symbol {
  let br = self.bit_reader();
  let bits = bit_reader::BrotliGet16BitsUnmasked(br, input);
  let mut table_index = bits & ((1 << l1numbits) - 1);
  let mut table_element = fast!((table)[table_index as usize]);
  if table_element.bits > l1numbits {
    let nbits = table_element.bits - l1numbits as u8;
    bit_reader::BrotliDropBits(br, l1numbits.into());
    table_index += table_element.value as u32;
    table_element = fast!((table)[(table_index
                           + ((bits >> l1numbits)
                              & bit_reader::BitMask(nbits as u32))) as usize]);
  }
  bit_reader::BrotliDropBits(br, table_element.bits as u32);
  Symbol::cast(table_element.value)

  }
  // precondition: input has at least 4 bytes
  fn safe_get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, l1numbits: u8, input: &[u8], is_speculative: Speculative) -> (Symbol, BrotliResult){
    if !self.active {
      return (Symbol::from(0), BrotliResult::ResultSuccess);
    }
    let mut ix: u32 = 0;
    if !bit_reader::BrotliSafeGetBits(self.bit_reader(), l1numbits.into(), &mut ix, input) {
      let available_bits: u32 = bit_reader::BrotliGetAvailableBits(self.br());
      if available_bits != 0 {
        ix = bit_reader::BrotliGetBitsUnmasked(self.br()) as u32 & ((1 << l1numbits) - 1);
      } else {
        ix = 0;
      }
      if group[ix as usize].bits as u32 > available_bits {
        return (Symbol::from(0), BrotliResult::NeedsMoreInput);
      }
    }
    let entry = group[ix as usize];
    if (entry.value as usize) < Spec::ALPHABET_SIZE {
      bit_reader::BrotliDropBits(self.bit_reader(), entry.bits as u32);
      return (Symbol::cast(entry.value), BrotliResult::ResultSuccess);
    }
    if !bit_reader::BrotliSafeGetBits(self.bit_reader(), entry.bits as u32, &mut ix, input) {
        return (Symbol::from(0), BrotliResult::NeedsMoreInput);
    }
    bit_reader::BrotliDropBits(self.bit_reader(), entry.bits as u32);
    (Symbol::cast(group[ix as usize + entry.value as usize].value), BrotliResult::ResultSuccess)
  }
  // precondition: input has at least 4 bytes
  fn get_uniform<Speculative:BoolTrait>(&mut self, nbits: u8, input: &[u8], is_speculative:Speculative) -> (u32, BrotliResult){
    let mut ix = 0u32;
    if self.active {
      if !bit_reader::BrotliSafeReadBits(&mut self.br, u32::from(nbits), &mut ix, input) {
        
        return (0, BrotliResult::NeedsMoreInput)
      }
    }
    (ix, BrotliResult::ResultSuccess)
  }
  type SpeculativeState = bit_reader::BrotliBitReaderState;
  fn placeholder(&self) -> Self::SpeculativeState{
    bit_reader::BrotliBitReaderState::default()
  }
  fn begin_speculative(&mut self) -> Self::SpeculativeState{
    bit_reader::BrotliBitReaderSaveState(self.br())
  }
  fn commit_speculative(&mut self){}
  fn abort_speculative(&mut self, val:Self::SpeculativeState){
    if self.active {
      bit_reader::BrotliBitReaderRestoreState(self.bit_reader(),&val);
    }
  }
}

#[derive(Default)]
pub struct NopEncoder {
}

impl EntropyEncoder for NopEncoder {
  fn put<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocU32:Allocator<u32>,AllocHC:Allocator<HuffmanCode>, Spec:HistogramSpec>(&mut self, group:HuffmanTreeGroup<AllocU32, AllocHC>, prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, prior: u8, symbol: Symbol, output:&mut [u8], output_offset:&mut usize) -> BrotliResult {
    BrotliResult::ResultSuccess
  }
  fn put_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, symbol: Symbol, output: &mut[u8], output_offset:&mut usize) -> BrotliResult {
    BrotliResult::ResultSuccess
  }
  fn put_uniform(&mut self, nbits: u8, symbol: u16, output: &mut [u8], output_offset: &mut usize){}
  fn flush(&mut self, output: &mut[u8], output_offset:&mut usize) -> BrotliResult {
    BrotliResult::ResultSuccess
  }
}
