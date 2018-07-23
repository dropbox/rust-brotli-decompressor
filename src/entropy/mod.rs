use super::{HuffmanCode, HuffmanTreeGroup};
use super::huffman::histogram::{ANSTable, HistogramSpec};
use super::BrotliResult;
use core::ops::AddAssign;
use alloc;
use alloc::Allocator;
use alloc::SliceWrapper;
use alloc::SliceWrapperMut;

pub trait EntropyEncoder {
    fn put<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocU32:Allocator<u32>,AllocHC:Allocator<HuffmanCode>, Spec:HistogramSpec>(&mut self, group:HuffmanTreeGroup<AllocU32, AllocHC>, prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, prior: u8, symbol: Symbol, output:&mut [u8], output_offset:&mut usize) -> BrotliResult;
    fn put_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, symbol: Symbol, output: &mut[u8], output_offset:&mut usize) -> BrotliResult;
    fn flush(&mut self, output: &mut[u8], output_offset:&mut usize) -> BrotliResult;
}

pub trait EntropyDecoder {
    // precondition: input has at least 4 bytes
    fn get<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocU32:Allocator<u32>, AllocHC:Allocator<HuffmanCode>, Spec:HistogramSpec>(&mut self, group:HuffmanTreeGroup<AllocU32, AllocHC>, prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, prior: u8, input:&[u8], input_offset:&mut usize) -> BrotliResult;
    // precondition: input has at least 4 bytes
    fn get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, input: &[u8], input_offset:&mut usize) -> (Symbol, BrotliResult);    
}


#[derive(Default)]
pub struct HuffmanDecoder {
    
}

impl EntropyDecoder  for HuffmanDecoder {
    fn get<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocU32:Allocator<u32>, AllocHC:Allocator<HuffmanCode>, Spec:HistogramSpec>(&mut self, group:HuffmanTreeGroup<AllocU32, AllocHC>, prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, prior: u8, input:&[u8], input_offset:&mut usize) -> BrotliResult{
        BrotliResult::ResultFailure
    }
    // precondition: input has at least 4 bytes
    fn get_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, input: &[u8], input_offset:&mut usize) -> (Symbol, BrotliResult) {
        (Symbol::from(0), BrotliResult::ResultFailure)
    }
}

#[derive(Default)]
pub struct NopEncoder {
}

impl EntropyEncoder for NopEncoder {
    fn put<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocU32:Allocator<u32>,AllocHC:Allocator<HuffmanCode>, Spec:HistogramSpec>(&mut self, group:HuffmanTreeGroup<AllocU32, AllocHC>, prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, prior: u8, symbol: Symbol, output:&mut [u8], output_offset:&mut usize) -> BrotliResult {
        BrotliResult::ResultSuccess
    }
    fn put_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8> + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, Spec:HistogramSpec>(&mut self, group:&[HuffmanCode], prob: &ANSTable<u32, Symbol, AllocS, AllocH, Spec>, symbol: Symbol, output: &mut[u8], output_offset:&mut usize) -> BrotliResult {
        BrotliResult::ResultSuccess
    }
    fn flush(&mut self, output: &mut[u8], output_offset:&mut usize) -> BrotliResult {
        BrotliResult::ResultSuccess
    }
}
