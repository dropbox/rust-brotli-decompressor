use super::super::bit_reader;
use ::BrotliResult;
use core::marker::PhantomData;
use alloc;
use alloc::Allocator;
use super::super::{HuffmanCode, HuffmanTreeGroup};
use super::super::huffman::histogram::{ANSTable, HistogramSpec};
use super::interface::*;
use core::ops::AddAssign;
use entropy::log4096::LOG4096;
#[derive(Default)]
pub struct BillingEncoder {
  total: f64,
  spec: f64,
}

#[allow(unused)]
impl<AllocU8:Allocator<u8>,AllocU32: Allocator<u32>> EntropyEncoder<AllocU8, AllocU32> for BillingEncoder {
  fn put<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait> (
    &mut self,
    m8: &mut AllocU8, m32: &mut AllocU32,
    group:&[&[HuffmanCode];256],
    prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
    prior: (u8, u8, u8),
    mut symbol: Symbol,
    is_speculative: Speculative){
    let mut hist_ent = prob.get_prob(prior.0, symbol.into_u64() as u32);
    assert!(hist_ent.freq() != 0);
    let val = LOG4096[hist_ent.freq() as usize];
    if Speculative::VALUE {
      self.spec -= val;
    } else {
      self.total -= val;
    }
  }
  fn put_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait>(
    &mut self,
    m8: &mut AllocU8, m32: &mut AllocU32,
    group:&[HuffmanCode],
    prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
    l1numbits: u8,
    symbol: Symbol, 
    speculative: Speculative) {
    let mut hist_ent = prob.get_prob(0, symbol.into_u64() as u32);
    assert!(hist_ent.freq() != 0);
    let val = LOG4096[hist_ent.freq() as usize];
    if Speculative::VALUE {
      self.spec -= val;
    } else {
      self.total -= val;
    }
  }
  fn put_uniform<Speculative:BoolTrait> (
    &mut self,
    m8: &mut AllocU8, m32: &mut AllocU32,
    nbits: u8,
    symbol: u32,
    is_speculative: Speculative) {
    if Speculative::VALUE {
      self.spec += nbits as f64;
    } else {
      self.total += nbits as f64;
    }    
  }
  fn begin_speculative(&mut self){}
  fn commit_speculative(&mut self){
    self.total += self.spec;
    self.spec = 0.0;
  }
  fn abort_speculative(&mut self){
    self.spec = 0.0;
  }
  fn drain(&mut self, out_data: &mut [u8]) -> usize {0}
  fn finish(&mut self, out_data:&mut [u8]) -> usize {
    eprintln!("Total: {} bits, {} bytes", self.total, self.total / 8.0);
    0
  }
}

