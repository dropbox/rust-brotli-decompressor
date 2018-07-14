use core;
use core::ops::AddAssign;
use core::cmp::{Ord, min, max};
use core::convert::From;
use alloc;
use alloc::Allocator;
use alloc::SliceWrapper;
use alloc::SliceWrapperMut;
use super::HuffmanCode;
use super::HuffmanTreeGroup;
#[allow(unused)]
pub type Freq = u16;
#[allow(unused)]
const TF_SHIFT: Freq = 12;
const TOT_FREQ: Freq = 1 << TF_SHIFT;

#[derive(Copy,Clone,Default)]
pub struct HistEnt(pub u32);

impl HistEnt {
    #[inline(always)]
    pub fn start(&self) -> Freq {
        (self.0 & 0xffff) as Freq
    }
    #[inline(always)]
    pub fn freq(&self) -> Freq {
        (self.0 >> 16) as Freq
    }
    #[inline(always)]
    pub fn set_start(&mut self, start:Freq) -> HistEnt {
        self.0 &= 0xffff0000;
        self.0 |= start as u32;
        *self
    }
    #[inline(always)]
    pub fn set_freq(&mut self, freq:Freq) -> HistEnt {
        self.0 &= 0xffff;
        self.0 |= (freq as u32) << 16;
        *self
    }
}

impl From<u32> for HistEnt {
    fn from(data:u32) -> HistEnt {
        HistEnt(data)
    }
}

impl Into<u32> for HistEnt {
    fn into(self) -> u32 {
        self.0
    }
}


pub trait HistogramSpec:Default {
    const ALPHABET_SIZE:usize;
    const MAX_SYMBOL:u64;
    
}

struct Histogram<AllocU32:Allocator<u32>, Spec:HistogramSpec> {
    histogram: AllocU32::AllocatedMemory,
    num_htrees: u16,
    spec:Spec,
}

impl<AllocH:Allocator<u32>, Spec:HistogramSpec> Histogram<AllocH, Spec> {
    fn new<AllocU32: Allocator<u32>, AllocHC:Allocator<HuffmanCode>>(alloc_u32: &mut AllocH,  group:&HuffmanTreeGroup<AllocU32,AllocHC>) -> Self{
        let mut  ret = Histogram::<AllocH, Spec> {
            num_htrees:group.num_htrees,
            histogram:alloc_u32.alloc_cell(Spec::ALPHABET_SIZE * group.num_htrees as usize),
            spec:Spec::default(),
        };
        for cur_htree in 0..group.num_htrees {
            let mut total_count = 0u32;
            // lets collect samples from each htree
            let start = group.htrees.slice()[cur_htree as usize] as usize;
            let next_htree = cur_htree as usize + 1;
            let end = if next_htree < group.htrees.len() {
                group.htrees.slice()[next_htree] as usize
            } else {
                group.codes.len()
            };
            for (index, code) in group.codes.slice()[start..end].iter().enumerate() {
                let count = (index < 256) as u32 * 255 + 1;
                total_count += count;
                let sym = code.value;
                if u64::from(code.value) <= Spec::MAX_SYMBOL {
                    ret.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize] +=  count;
                }
            }
            assert_eq!(total_count, 65536);
        }
        ret.renormalize();    
        ret
    }
    fn renormalize(&mut self) {
        let shift = 4;
        let multiplier = 1 << shift;
        assert_eq!(65536/TOT_FREQ, 1<<shift);
        for cur_htree in 0..self.num_htrees {
            // precondition: table adds up to 65536
            let mut total_count = 0u32;
            for sym in 0..Spec::ALPHABET_SIZE {
                total_count += self.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize];
            }
            assert_eq!(total_count, 65536);
            let mut delta = 0i32;
            for sym in 0..Spec::ALPHABET_SIZE {
                let cur = &mut self.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize];
                let mut div = *cur >> shift;
                let rem = *cur & (multiplier - 1);
                if *cur != 0 {
                    if div == 0 {
                        div = 1;
                        delta += (multiplier - rem) as i32;
                    } else {
                        if rem >= multiplier / 2 {
                            div += 1;
                            delta += (multiplier - rem) as i32;
                        } else {
                            delta -= rem as i32;
                        }
                    }
                    *cur = div;
                }
            }
            assert_eq!(if delta < 0 {(-delta) as u32} else {delta as u32 } & (multiplier as u32 - 1), 0);
            delta /= multiplier as i32;
            for sym in 0..Spec::ALPHABET_SIZE {
                let cur = &mut self.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize];
                if *cur != 0 {
                    if delta < 0 {
                        *cur += 1;
                        delta +=1;
                    }
                    if delta > 0 && *cur > 1 {
                        *cur -= 1;
                        delta -=1;
                    }
                }
            }
            assert!(delta >= 0);
            if delta != 0 {
                for sym in 0..Spec::ALPHABET_SIZE {
                    let cur = &mut self.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize];
                    if *cur != 0 {
                        if delta > 0 && *cur > 1 {
                            let adj = min(*cur - 1, delta as u32);
                            *cur -= adj;
                            delta -= adj as i32;
                        }
                    }
                    if delta == 0 {
                        break;
                    }
                }
            }
            assert_eq!(delta, 0);
            let mut total_count = 0u32;
            for cur_htree in 0..self.num_htrees {
                for sym in 0..Spec::ALPHABET_SIZE {
                    total_count += self.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize];
                }
            }
            // postcondition: table adds up to TOT_FREQ
            assert_eq!(total_count, u32::from(TOT_FREQ));
        }
    }
    fn free(&mut self, m32: &mut AllocH) {
        m32.free_cell(core::mem::replace(&mut self.histogram, AllocH::AllocatedMemory::default()));
        
    }
}
pub struct ANSTable<HistEntTrait, Symbol:Sized+Ord+AddAssign<Symbol>+From<u8>+Clone, AllocS: Allocator<Symbol>, AllocH: Allocator<HistEntTrait>, Spec:HistogramSpec>
    where HistEnt:From<HistEntTrait> {
    state_lookup:AllocS::AllocatedMemory,
    sym:AllocH::AllocatedMemory,
    spec: Spec,
    num_htrees: u16,
}
impl<Symbol:Sized+Ord+AddAssign<Symbol>+From<u8>+Clone,
     AllocS: Allocator<Symbol>,
     AllocH: Allocator<u32>, Spec:HistogramSpec> ANSTable<u32, Symbol, AllocS, AllocH, Spec> {
    pub fn new<AllocU32:Allocator<u32>, AllocHC:Allocator<HuffmanCode>>(alloc_u8: &mut AllocS, alloc_u32: &mut AllocH, group:&HuffmanTreeGroup<AllocU32, AllocHC>, spec: Spec) -> Self {
        let mut histogram = Histogram::<AllocH, Spec>::new(alloc_u32, group);
        let mut rev_lookup = alloc_u8.alloc_cell(histogram.num_htrees as usize * TOT_FREQ as usize);
        for cur_htree in 0..group.num_htrees {
            let mut running_start = 0;
            let mut sym = Symbol::from(0u8);
            for start_freq in histogram.histogram.slice_mut().split_at_mut(
                cur_htree as usize * Spec::ALPHABET_SIZE).1.split_at_mut(Spec::ALPHABET_SIZE).0.iter_mut() {
                let count = *start_freq;
                *start_freq = HistEnt::default().set_start(running_start as u16).set_freq(count as u16).into();
                if count != 0 {
                    let running_end = running_start as usize + count as usize;
                    for rev_lk in rev_lookup.slice_mut()[running_start as usize..running_end].iter_mut() {
                        *rev_lk = sym.clone();
                    }
                    running_start = running_end;
                }
            }
            sym += Symbol::from(1u8);
        }
        ANSTable::<u32, Symbol, AllocS, AllocH, Spec>{
            state_lookup:rev_lookup,
            sym:histogram.histogram,
            spec:spec,
            num_htrees:histogram.num_htrees
        }
    }
    fn free(&mut self, ms: &mut AllocS, mh: &mut AllocH) {
        ms.free_cell(core::mem::replace(&mut self.state_lookup, AllocS::AllocatedMemory::default()));
        mh.free_cell(core::mem::replace(&mut self.sym, AllocH::AllocatedMemory::default()));
    }
}
#[derive(Clone,Copy,Default)]
pub struct LiteralSpec{}
impl HistogramSpec for LiteralSpec {
    const ALPHABET_SIZE: usize = 256;
    const MAX_SYMBOL: u64 = 0xff;
}
#[derive(Clone,Copy,Default)]
pub struct DistanceSpec{}
impl HistogramSpec for DistanceSpec {
    const ALPHABET_SIZE: usize = 704;
    const MAX_SYMBOL: u64 = 703;
}
#[derive(Clone,Copy,Default)]
pub struct BlockLengthSpec{}
impl HistogramSpec for BlockLengthSpec {
    const ALPHABET_SIZE: usize = 26;
    const MAX_SYMBOL: u64 = 25;
}
#[derive(Clone,Copy,Default)]
pub struct InsertCopySpec{}
impl HistogramSpec for InsertCopySpec {
    const ALPHABET_SIZE: usize = 704;
    const MAX_SYMBOL: u64 = 703;
}
struct LiteralANSTable<AllocSym:Allocator<u8>, AllocH:Allocator<u32>>(ANSTable<u32, u8, AllocSym,  AllocH, LiteralSpec>);

struct DistanceANSTable<AllocSym:Allocator<u16>, AllocH:Allocator<u32>>(ANSTable<u32, u16, AllocSym,  AllocH, DistanceSpec>);
struct InsertCopyANSTable<AllocSym:Allocator<u16>, AllocH:Allocator<u32>>(ANSTable<u32, u16, AllocSym,  AllocH, InsertCopySpec>);
