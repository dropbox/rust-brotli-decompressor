use super::super::bit_reader;
use ::BrotliResult;
use core::marker::PhantomData;
use alloc;
use alloc::Allocator;
use super::super::{HuffmanCode, HuffmanTreeGroup};
use super::super::huffman::histogram::{ANSTable, HistogramSpec};
use super::interface::*;
use core::ops::AddAssign;

pub const HUFFMAN_TABLE_BITS: u32 = 8;
pub const HUFFMAN_TABLE_MASK: u32 = 0xff;

pub struct HuffmanDecoder<AllocU8:Allocator<u8>, AllocU32:Allocator<u32>> {
    br: bit_reader::BrotliBitReader,
    _p8: PhantomData<AllocU8>,
    _p32: PhantomData<AllocU32>,
}

impl<AllocU8: Allocator<u8>, AllocU32: Allocator<u32>> Default for HuffmanDecoder<AllocU8, AllocU32> {
    fn default() -> Self {
        HuffmanDecoder::<AllocU8, AllocU32> {
            br: bit_reader::BrotliBitReader::default(),
            _p8: PhantomData::default(),
            _p32: PhantomData::default(),
        }
    }
}

impl<AllocU8: Allocator<u8>, AllocU32: Allocator<u32>>  EntropyDecoder<AllocU8, AllocU32> for HuffmanDecoder<AllocU8, AllocU32> {
  fn bit_reader(&mut self) -> &mut bit_reader::BrotliBitReader {
    &mut self.br
  }
  fn br(&self) -> &bit_reader::BrotliBitReader {
    &self.br
  }
  fn warmup(&mut self, input:&[u8]) -> BrotliResult{
    if (!bit_reader::BrotliWarmupBitReader(&mut self.br,
                                           input)) {
        return BrotliResult::NeedsMoreInput;
    }
    BrotliResult::ResultSuccess
  }
  fn begin_metablock(&mut self, _input:&[u8]) -> BrotliResult{
    // nothing to do for standard huffman-coded items
    BrotliResult::ResultSuccess
  }
  fn sufficient_bits(&mut self, nbits: u8) -> bool{
    bit_reader::BrotliCheckInputAmount(self.br(), nbits.into())
  }
  fn preload<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
             AllocS:Allocator<Symbol>,
             AllocH: Allocator<u32>,
             Spec:HistogramSpec>(&mut self,
                                 m8: &mut AllocU8, m32: &mut AllocU32,
                                 group:&[&[HuffmanCode];256],
                                 _prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                 prior: u8,
                                 input:&[u8]) -> (u32, u32){
    let table_element =
      fast!((group[usize::from(prior)])[bit_reader::BrotliGetBits(&mut self.br, HUFFMAN_TABLE_BITS, input) as usize]);
    (u32::from(table_element.bits),
     u32::from(table_element.value))
  }
  // precondition: input has at least 4 bytes
  fn get_preloaded<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
         AllocS:Allocator<Symbol>,
         AllocH: Allocator<u32>,
         Spec:HistogramSpec>(&mut self,
                                 m8: &mut AllocU8, m32: &mut AllocU32,
                                group:&[&[HuffmanCode];256],
                                _prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                prior: u8,
                                preloaded: (u32, u32),
                                input:&[u8]) -> Symbol {
      let result = if preloaded.0 > HUFFMAN_TABLE_BITS {
        let val = bit_reader::BrotliGet16BitsUnmasked(&mut self.br, input);
        let mut ext_index = (val & HUFFMAN_TABLE_MASK) + preloaded.1;
        let mask = bit_reader::BitMask((preloaded.0 - HUFFMAN_TABLE_BITS));
        bit_reader::BrotliDropBits(&mut self.br, HUFFMAN_TABLE_BITS);
        ext_index += (val >> HUFFMAN_TABLE_BITS) & mask;
        let ext = fast!((group[usize::from(prior)])[ext_index as usize]);
        bit_reader::BrotliDropBits(&mut self.br, ext.bits as u32);
        Symbol::cast(ext.value)
      } else {
        bit_reader::BrotliDropBits(&mut self.br, preloaded.0);
        Symbol::cast(preloaded.1 as u16)
      };
    result

  }
  fn get<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone,
         AllocS:Allocator<Symbol>,
         AllocH: Allocator<u32>,
         Spec:HistogramSpec,
         Speculative:BoolTrait>(&mut self,
                                 m8: &mut AllocU8, m32: &mut AllocU32,
                                group:&[&[HuffmanCode];256],
                                _prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
                                prior: u8,
                                input:&[u8],
                                _is_speculative: Speculative) -> (Symbol, BrotliResult) {
    let mut available_bits;
    let mut val = 0;
    if bit_reader::BrotliSafeGetBits(&mut self.br, 15, &mut val, input) {
        let mut table_index = val & HUFFMAN_TABLE_MASK;
        let mut table_element = fast!((group[usize::from(prior)])[table_index as usize]);
        if table_element.bits > HUFFMAN_TABLE_BITS as u8 {
            let nbits = table_element.bits - HUFFMAN_TABLE_BITS as u8;
            bit_reader::BrotliDropBits(&mut self.br, HUFFMAN_TABLE_BITS);
            table_index += table_element.value as u32;
            table_element = fast!((group[usize::from(prior)])[(table_index
                                           + ((val >> HUFFMAN_TABLE_BITS)
                                              & bit_reader::BitMask(nbits as u32))) as usize]);
            
        }
        bit_reader::BrotliDropBits(&mut self.br, table_element.bits as u32);
        return (Symbol::cast(table_element.value), BrotliResult::ResultSuccess);
    } else {
        available_bits = bit_reader::BrotliGetAvailableBits(&mut self.br);
    }
  
    if (available_bits == 0) {
      if (fast!((group[usize::from(prior)])[0]).bits == 0) {
        return (Symbol::cast(fast!((group[usize::from(prior)])[0]).value), BrotliResult::ResultSuccess);
      }
      return (Symbol::from(0), BrotliResult::NeedsMoreInput);
    }
    let mut val = bit_reader::BrotliGetBitsUnmasked(&mut self.br) as u32;
    let table_index = (val & HUFFMAN_TABLE_MASK) as usize;
    let table_element = fast!((group[usize::from(prior)])[table_index]);
    if (table_element.bits <= HUFFMAN_TABLE_BITS as u8) {
      if (table_element.bits as u32 <= available_bits) {
        bit_reader::BrotliDropBits(&mut self.br, table_element.bits as u32);
        return (Symbol::cast(table_element.value), BrotliResult::ResultSuccess);
      } else {
        return (Symbol::from(0), BrotliResult::NeedsMoreInput);
      }
    }
    if (available_bits <= HUFFMAN_TABLE_BITS) {
      return (Symbol::from(0), BrotliResult::NeedsMoreInput); /* Not enough bits to move to the second level. */
    }

    // Speculatively drop HUFFMAN_TABLE_BITS.
    val = (val & bit_reader::BitMask(table_element.bits as u32)) >> HUFFMAN_TABLE_BITS;
    available_bits -= HUFFMAN_TABLE_BITS;
    let table_sub_element = fast!((group[usize::from(prior)])[table_index + table_element.value as usize + val as usize]);
    if (available_bits < table_sub_element.bits as u32) {
      return (Symbol::from(0), BrotliResult::NeedsMoreInput); /* Not enough bits for the second level. */
    }
    bit_reader::BrotliDropBits(&mut self.br, HUFFMAN_TABLE_BITS + table_sub_element.bits as u32);
    (Symbol::cast(table_sub_element.value), BrotliResult::ResultSuccess)
  }
  fn get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(
      &mut self,
      m8: &mut AllocU8, m32: &mut AllocU32,
      table:&[HuffmanCode],
      _prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
      l1numbits: u8,
      input: &[u8],
) -> Symbol {
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
    fn safe_get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec, Speculative:BoolTrait>(
        &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        group:&[HuffmanCode],
        _prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>,
        l1numbits: u8,
        input: &[u8],
        _is_speculative: Speculative,
) -> (Symbol, BrotliResult){
    let mut val: u32 = 0;
    let mut available_bits;
      
    if bit_reader::BrotliSafeGetBits(&mut self.br, 15, &mut val, input) {
        let mut table_index = val & ((1 << l1numbits) - 1);
        let mut table_element = fast!((group)[table_index as usize]);
        if table_element.bits > l1numbits as u8 {
            let nbits = table_element.bits - l1numbits as u8;
            bit_reader::BrotliDropBits(&mut self.br, u32::from(l1numbits));
            table_index += table_element.value as u32;
            table_element = fast!((group)[(table_index
                                           + ((val >> l1numbits)
                                              & bit_reader::BitMask(nbits as u32))) as usize]);
            
        }
        bit_reader::BrotliDropBits(&mut self.br, table_element.bits as u32);
        return (Symbol::cast(table_element.value), BrotliResult::ResultSuccess);
    } else {
        available_bits = bit_reader::BrotliGetAvailableBits(&mut self.br);
    }
  
    if (available_bits == 0) {
      if (fast!((group)[0]).bits == 0) {
        return (Symbol::cast(fast!((group)[0]).value), BrotliResult::ResultSuccess);
      }
      return (Symbol::from(0), BrotliResult::NeedsMoreInput);
    }
    let mut val = bit_reader::BrotliGetBitsUnmasked(&mut self.br) as u32;
    let table_index = (val & ((1 << l1numbits) - 1)) as usize;
    let table_element = fast!((group)[table_index]);
    if (table_element.bits <= l1numbits) {
      if (table_element.bits as u32 <= available_bits) {
        bit_reader::BrotliDropBits(&mut self.br, table_element.bits as u32);
        return (Symbol::cast(table_element.value), BrotliResult::ResultSuccess);
      } else {
        return (Symbol::from(0), BrotliResult::NeedsMoreInput);
      }
    }
    if (available_bits <= u32::from(l1numbits)) {
      return (Symbol::from(0), BrotliResult::NeedsMoreInput); /* Not enough bits to move to the second level. */
    }

    // Speculatively drop l1numbits.
    val = (val & bit_reader::BitMask(table_element.bits as u32)) >> l1numbits;
    available_bits -= u32::from(l1numbits);
    let table_sub_element = fast!((group)[table_index + table_element.value as usize + val as usize]);
    if (available_bits < table_sub_element.bits as u32) {
      return (Symbol::from(0), BrotliResult::NeedsMoreInput); /* Not enough bits for the second level. */
    }
    bit_reader::BrotliDropBits(&mut self.br, u32::from(l1numbits) + u32::from(table_sub_element.bits));
    (Symbol::cast(table_sub_element.value), BrotliResult::ResultSuccess)
  }
  // precondition: input has at least 4 bytes
    fn get_uniform<Speculative:BoolTrait>(
       &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        nbits: u8, input: &[u8], _is_speculative:Speculative) -> (u32, BrotliResult){
    if nbits == 0 {
        return (0, BrotliResult::ResultSuccess);
    }
    let mut ix = 0u32;
    if !bit_reader::BrotliSafeReadBits(&mut self.br, u32::from(nbits), &mut ix, input) {
        
        return (0, BrotliResult::NeedsMoreInput)
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
     bit_reader::BrotliBitReaderRestoreState(self.bit_reader(),&val);
  }
}

