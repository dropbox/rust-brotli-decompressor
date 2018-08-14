use core::marker::PhantomData;
use super::super::bit_reader;
use ::BrotliResult;
use alloc;
use alloc::Allocator;
use super::super::{HuffmanCode, HuffmanTreeGroup};
use super::super::huffman::histogram::{ANSTable, HistogramSpec, FrequentistCDF};
use core::ops::AddAssign;
use super::interface::*;
pub struct Tee<AllocU8:Allocator<u8>,
               AllocU32:Allocator<u32>,
               Decoder:EntropyDecoder<AllocU8, AllocU32>,
               Encoder:EntropyEncoder<AllocU8, AllocU32>> {
  decoder: Decoder,
  encoder: Encoder,
  p0: PhantomData<AllocU8>,
  p1: PhantomData<AllocU32>,
}
impl<AllocU8:Allocator<u8>, AllocU32:Allocator<u32>,
     Decoder:EntropyDecoder<AllocU8, AllocU32>,
     Encoder:EntropyEncoder<AllocU8, AllocU32>> Tee<AllocU8, AllocU32, Decoder, Encoder> {
  fn new(decoder: Decoder, encoder: Encoder) -> Self {
    Tee{
      decoder:decoder,
      encoder:encoder,
      p0: PhantomData::default(),
      p1: PhantomData::default(),
    }
  }
}
impl<AllocU8:Allocator<u8>, AllocU32:Allocator<u32>,
     Decoder:EntropyDecoder<AllocU8, AllocU32>+Default,
     Encoder:EntropyEncoder<AllocU8, AllocU32>+Default> Default for Tee<AllocU8, AllocU32, Decoder, Encoder> {
  fn default() -> Self {
    Tee{
      decoder:Decoder::default(),
      encoder:Encoder::default(),
      p0:PhantomData::default(),
      p1:PhantomData::default(),
    }
  }
}

impl<AllocU8:Allocator<u8>, AllocU32:Allocator<u32>,
     Decoder:EntropyDecoder<AllocU8, AllocU32>,
     Encoder:EntropyEncoder<AllocU8, AllocU32>> EntropyDecoder<AllocU8, AllocU32> for Tee<AllocU8, AllocU32, Decoder, Encoder> {
  type SpeculativeState = Decoder::SpeculativeState;
  fn bit_reader(&mut self) -> &mut bit_reader::BrotliBitReader {
    self.decoder.bit_reader()
  }
  fn br(&self) -> &bit_reader::BrotliBitReader {
    self.decoder.br()
  }
  fn warmup(&mut self, input:&[u8]) -> BrotliResult {
    self.decoder.warmup(input)
  }
  fn begin_metablock(&mut self, input:&[u8]) -> BrotliResult {
    self.decoder.begin_metablock(input)
  }
  fn sufficient_bits(&mut self, nbits: u8) -> bool{
    self.decoder.sufficient_bits(nbits)
  }
  fn placeholder(&self) -> Self::SpeculativeState {
    self.decoder.placeholder()
  }
  fn preload<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
             AllocS:Allocator<Symbol>,
             AllocH: Allocator<u32>,
             AllocCDF : Allocator<FrequentistCDF>,
             Spec:HistogramSpec>(&mut self,
                                 m8: &mut AllocU8, m32: &mut AllocU32,
                                 group:&[&[HuffmanCode];256],
                                 prob: &ANSTable<u32, Symbol, AllocS, AllocH, AllocCDF, Spec>,
                                 prior: (u8, u8, u8),
                                 input:&[u8]) -> (u32, u32){
    self.decoder.preload(m8, m32, group, prob, prior, input)
  }
  // precondition: input has at least 4 bytes
  fn get_preloaded<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
                   AllocS:Allocator<Symbol>,
                   AllocH: Allocator<u32>,
                   AllocCDF : Allocator<FrequentistCDF>,
                   Spec:HistogramSpec>(&mut self,
                                       m8: &mut AllocU8, m32: &mut AllocU32,
                                       group:&[&[HuffmanCode];256],
                                       prob: &ANSTable<u32, Symbol, AllocS, AllocH, AllocCDF, Spec>,
                                       prior: (u8, u8, u8),
                                       preloaded: (u32, u32),
                                       input:&[u8]) -> Symbol {
    let ret = self.decoder.get_preloaded(m8, m32, group, prob, prior, preloaded, input);
    self.encoder.put(m8, m32, group, prob, prior, ret.clone(), Unconditional{});
    ret
  }
  // precondition: input has at least 4 bytes
  fn get<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
         AllocS:Allocator<Symbol>,
         AllocH: Allocator<u32>,
         AllocCDF : Allocator<FrequentistCDF>,
         Spec:HistogramSpec,
         Speculative:BoolTrait>(&mut self,
                                m8: &mut AllocU8, m32: &mut AllocU32,
                                group:&[&[HuffmanCode];256],
                                prob: &ANSTable<u32, Symbol, AllocS, AllocH, AllocCDF, Spec>,
                                prior: (u8, u8, u8),
                                input:&[u8],
                                is_speculative: Speculative) -> (Symbol, BrotliResult) {
    let (sym, res) = self.decoder.get(m8, m32, group, prob, prior, input, is_speculative);
    if let BrotliResult::ResultSuccess = res {
      self.encoder.put(m8, m32, group, prob, prior, sym.clone(), is_speculative)
    }
    (sym, res)
  }
    fn get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>,  AllocCDF : Allocator<FrequentistCDF>, Spec:HistogramSpec>(
      &mut self,
      m8: &mut AllocU8, m32: &mut AllocU32,
      group:&[HuffmanCode],
      prob: &ANSTable<u32, Symbol, AllocS, AllocH, AllocCDF, Spec>,
      l1numbits: u8,
      input:&[u8]) -> Symbol {
      let sym = self.decoder.get_stationary(m8, m32, group, prob, l1numbits, input);
      self.encoder.put_stationary(m8, m32, group, prob, l1numbits, sym.clone(), Unconditional{});
      sym
    }
  // precondition: input has at least 4 bytes
    fn safe_get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocCDF : Allocator<FrequentistCDF>, Spec:HistogramSpec, Speculative:BoolTrait>(
        &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        group:&[HuffmanCode],
        prob: &ANSTable<u32, Symbol, AllocS, AllocH, AllocCDF, Spec>,
        l1numbits: u8,
        input: &[u8],
      is_speculative: Speculative) -> (Symbol, BrotliResult) {
      let (sym, res) = self.decoder.safe_get_stationary(m8, m32,
                                                        group,
                                                        prob,
                                                        l1numbits,
                                                        input, is_speculative);
      if let BrotliResult::ResultSuccess = res {
        self.encoder.put_stationary(m8, m32, group, prob, l1numbits, sym.clone(), is_speculative);
      }
      (sym, res)
    }
    // precondition: input has at least 4 bytes
    fn get_uniform<Speculative:BoolTrait>(&mut self,
                                          m8: &mut AllocU8, m32: &mut AllocU32,
                                          nbits: u8, input: &[u8], is_speculative: Speculative) -> (u32, BrotliResult) {
      let (bits, res) = self.decoder.get_uniform(m8, m32, nbits, input, is_speculative);
      if let BrotliResult::ResultSuccess = res {
        self.encoder.put_uniform(m8, m32, nbits, bits, is_speculative);
      }
      (bits, res)
    }
    fn begin_speculative(&mut self) -> Self::SpeculativeState {
      self.encoder.begin_speculative();
      self.decoder.begin_speculative()
    }
    fn commit_speculative(&mut self){
      self.encoder.commit_speculative();
      self.decoder.commit_speculative();
    }
    fn abort_speculative(&mut self, val:Self::SpeculativeState) {
      self.decoder.abort_speculative(val);
      self.encoder.abort_speculative();
    }
    fn drain(&mut self, out_data: &mut [u8]) -> usize {
      let res = self.decoder.drain(out_data);
      self.encoder.drain(out_data) + res
    }
    fn finish(&mut self, out_data:&mut [u8]) -> usize {
      let res = self.decoder.finish(out_data);
      self.encoder.finish(out_data) + res
    }

}
