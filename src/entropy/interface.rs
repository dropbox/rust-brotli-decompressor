use super::super::{HuffmanCode, HuffmanTreeGroup};
use super::super::huffman::histogram::{ANSTable, HistogramSpec};
use super::super::BrotliResult;
use super::super::bit_reader;
use core::ops::AddAssign;
use core::marker::PhantomData;
use alloc;
use alloc::Allocator;
use alloc::SliceWrapper;
use alloc::SliceWrapperMut;


pub trait BoolTrait : Clone+Copy{
  const VALUE: bool;
}
#[derive(Clone, Copy)]
pub struct TrueBoolTrait {}
impl BoolTrait for TrueBoolTrait {
  const VALUE: bool = true;
}
#[derive(Clone, Copy)]
pub struct FalseBoolTrait{}
impl BoolTrait for FalseBoolTrait {
  const VALUE: bool = false;
}

pub type Speculative = TrueBoolTrait;
pub type Unconditional = FalseBoolTrait;

pub trait SymbolCast {
  fn cast(data: u16) -> Self;
  fn into_u64(&self) -> u64;
}

impl SymbolCast for u8 {
  fn cast(data:u16) -> Self {
    return data as u8
  }
  fn into_u64(&self) -> u64 {
    return u64::from(*self)
  }
}
impl SymbolCast for u16 {
  fn cast(data:u16) -> Self {
    return data
  }
  fn into_u64(&self) -> u64 {
    return u64::from(*self)
  }
}


pub trait EntropyEncoder<AllocU8: Allocator<u8>, AllocU32: Allocator<u32>> {
  fn put<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait>(
    &mut self,
    m8: &mut AllocU8, m32: &mut AllocU32,
    group:&[&[HuffmanCode];256],
    prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
    prior: (u8, u8, u8),
    symbol: Symbol,
    is_speculative: Speculative);
  fn put_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait>(
    &mut self,
    m8: &mut AllocU8, m32: &mut AllocU32,
    group:&[HuffmanCode],
    prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
    l1numbits: u8,
    symbol: Symbol,
    speculative: Speculative);
  fn put_uniform<Speculative:BoolTrait> (
    &mut self,
    m8: &mut AllocU8, m32: &mut AllocU32,
    nbits: u8,
    symbol: u32,
    is_speculative: Speculative);
  fn begin_speculative(&mut self);
  fn commit_speculative(&mut self);
  fn abort_speculative(&mut self);
  fn drain(&mut self, out_data: &mut [u8]) -> usize {0}
  fn finish(&mut self, out_data:&mut [u8]) -> usize {0}
}

use core::fmt;

pub trait EntropyDecoder<AllocU8: Allocator<u8>, AllocU32: Allocator<u32>> {
  type SpeculativeState: fmt::Debug+PartialEq+Eq;
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
                                 m8: &mut AllocU8, m32: &mut AllocU32,
                                 group:&[&[HuffmanCode];256],
                                 prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                 prior: (u8, u8, u8),
                                 input:&[u8]) -> (u32, u32);
  // precondition: input has at least 4 bytes
  fn get_preloaded<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
                   AllocS:Allocator<Symbol>,
                   AllocH: Allocator<u32>,
                   Spec:HistogramSpec>(&mut self,
                                       m8: &mut AllocU8, m32: &mut AllocU32,
                                       group:&[&[HuffmanCode];256],
                                       prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                       prior: (u8, u8, u8),
                                       preloaded: (u32, u32),
                                       input:&[u8]) -> Symbol;
  // precondition: input has at least 4 bytes
  fn get<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
         AllocS:Allocator<Symbol>,
         AllocH: Allocator<u32>,
         Spec:HistogramSpec,
         Speculative:BoolTrait>(&mut self,
                                m8: &mut AllocU8, m32: &mut AllocU32,
                                group:&[&[HuffmanCode];256],
                                prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                prior: (u8, u8, u8),
                                input:&[u8],
                                is_speculative: Speculative) -> (Symbol, BrotliResult);
    fn get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(
        &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        group:&[HuffmanCode],
        prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
        l1numbits: u8,
        input:&[u8]) -> Symbol;
  // precondition: input has at least 4 bytes
    fn safe_get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait>(
        &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        group:&[HuffmanCode],
        prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
        l1numbits: u8,
        input: &[u8],
        is_speculative: Speculative) -> (Symbol, BrotliResult);
    // precondition: input has at least 4 bytes
    fn get_uniform<Speculative:BoolTrait>(&mut self,
                                          m8: &mut AllocU8, m32: &mut AllocU32,
                                          nbits: u8, input: &[u8], is_speculative: Speculative) -> (u32, BrotliResult);
    fn begin_speculative(&mut self) -> Self::SpeculativeState;
    fn commit_speculative(&mut self);
    fn abort_speculative(&mut self, val:Self::SpeculativeState);
    fn drain(&mut self, out_data: &mut [u8]) -> usize {0}
    fn finish(&mut self, out_data:&mut [u8]) -> usize {0}
}

