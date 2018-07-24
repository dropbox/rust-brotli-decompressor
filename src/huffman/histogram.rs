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
}

struct Histogram<AllocU32:Allocator<u32>, Spec:HistogramSpec> {
    histogram: AllocU32::AllocatedMemory,
    num_htrees: u16,
    spec:Spec,
}
impl<AllocH:Allocator<u32>, Spec:HistogramSpec> Histogram<AllocH, Spec> {
    fn new_from_single_code<AllocU32: Allocator<u32>, AllocHC:Allocator<HuffmanCode>>(alloc_u32: &mut AllocH,  group:&HuffmanTreeGroup<AllocU32,AllocHC>, previous_mem: Option<AllocH::AllocatedMemory>) -> Self{
        Self::new(alloc_u32, &group.htrees.slice()[..group.num_htrees as usize], group.codes.slice(), previous_mem)
    }
    fn new(alloc_u32: &mut AllocH, group_count: &[u32], group_codes:&[HuffmanCode], previous_mem: Option<AllocH::AllocatedMemory>) -> Self{
        let num_htrees = group_count.len();
        let buf = if let Some(pmem) = previous_mem {
            assert!(pmem.len() >= Spec::ALPHABET_SIZE * num_htrees);
            pmem
        } else {
                
            alloc_u32.alloc_cell(Spec::ALPHABET_SIZE * num_htrees)
        };
        let mut ret = Histogram::<AllocH, Spec> {
            num_htrees:num_htrees as u16,
            histogram:buf,
            spec:Spec::default(),
        };
         for count in ret.histogram.slice_mut().iter_mut() {
            *count = 0;
        }
        for cur_htree in 0..num_htrees {
            let mut complete = [false;256];
            let mut total_count = 0u32;
            // lets collect samples from each htree
            let start = group_count[cur_htree] as usize;
            let next_htree = cur_htree + 1;
            let end = if next_htree < num_htrees {
                group_count[next_htree] as usize
            } else {
                group_codes.len()
            };
            if start == end || end == 0 {
                for sym in 0..256 {
                    ret.histogram.slice_mut()[cur_htree * Spec::ALPHABET_SIZE + sym as usize] = 256;
                    total_count += 256;
                }
            } else {
                for (index, code) in group_codes[start..core::cmp::min(start+256, end)].iter().enumerate() {
                    let count = (index < 256) as u32 * 255 + 1;
                    let sym = code.value;
                    let bits = code.bits;
                    if sym < Spec::ALPHABET_SIZE as u16 { //code.bits <= 8 {//&& !complete[(index & 0xff)] {
                        //assert!(bits <= 8);
                        ret.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize] +=  count;
                        total_count += count;
                    } else {
                        let count = 65536>>bits;
                        for sub_code in 0..(1<<(bits - 8)) {
                            let sub_index = start + index + sub_code + code.value as usize;
                            assert!(sub_index < end);
                            let sub_sym = group_codes[sub_index].value;
                            ret.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sub_sym as usize] +=  count;
                            total_count += count;
                            assert!(Spec::ALPHABET_SIZE > sub_sym as usize);
                        }
                    }
                    if index < 256 {
                        complete[index] = true;
                    }
                }
            }
            if total_count != TOT_FREQ as u32 {
                if total_count != 8192 {
                    assert_eq!(total_count, 65536);
                }
            }
        }
        ret.renormalize();    
        ret
    }
    fn renormalize(&mut self) {
        for cur_htree in 0..self.num_htrees {
            // precondition: table adds up to 65536
            let mut total_count = 0u32;
            for sym in 0..Spec::ALPHABET_SIZE {
                total_count += self.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize];
            }
            if total_count == TOT_FREQ as u32 {
                continue;
            }
            let shift;
            if total_count != 8192 {
                assert_eq!(total_count, 65536);
                shift = 4;
            } else {
                shift = 1;
            }
            let multiplier = 1 << shift;
            assert_eq!(total_count/TOT_FREQ as u32, 1u32<<shift);
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
            total_count = 0u32;
            for sym in 0..Spec::ALPHABET_SIZE {
                total_count += self.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize];
            }
            // postcondition: table adds up to TOT_FREQ
            assert_eq!(total_count, u32::from(TOT_FREQ));
        }
    }
    fn free(&mut self, m32: &mut AllocH) {
        m32.free_cell(core::mem::replace(&mut self.histogram, AllocH::AllocatedMemory::default()));
        
    }
}
pub struct CDF<HistEntTrait, AllocH: Allocator<HistEntTrait>, Spec:HistogramSpec> {
    sym:AllocH::AllocatedMemory,
    num_htrees: u16,
    spec: Spec,
}
impl<HistEntTrait, AllocH: Allocator<HistEntTrait>, Spec:HistogramSpec> Default for CDF<HistEntTrait, AllocH, Spec> {
    fn default() -> Self {
        CDF {
            sym:AllocH::AllocatedMemory::default(),
            spec:Spec::default(),
            num_htrees:0,
        }
    }
}
impl<AllocH: Allocator<u32>, Spec:HistogramSpec> CDF<u32, AllocH, Spec> {
    pub fn new_single_code(alloc_u32: &mut AllocH, group:&[HuffmanCode], spec: Spec, mut old_ans: Option<Self>) -> Self {
        Self::new(alloc_u32, &[0], group, spec, old_ans)
    }
    pub fn new(alloc_u32: &mut AllocH, group_count: &[u32], group:&[HuffmanCode], spec: Spec, mut old_ans: Option<Self>) -> Self {
        let desired_lt_size = Spec::ALPHABET_SIZE * group_count.len();
        let mut old_ans_ok = false;
        if let Some(ref mut old) = old_ans {
            if old.sym.len() >= desired_lt_size {
                old_ans_ok = true;
            }
        }
        
        let mut histogram;
        if old_ans_ok {
            let mut old = old_ans.unwrap();
            histogram = Histogram::<AllocH, Spec>::new(alloc_u32, group_count, group, Some(old.sym));
        } else {
            histogram = Histogram::<AllocH, Spec>::new(alloc_u32, group_count, group, None);
        }
        for cur_htree in 0..group_count.len() {
            let mut running_start = 0;
            for start_freq in histogram.histogram.slice_mut().split_at_mut(
                cur_htree as usize * Spec::ALPHABET_SIZE).1.split_at_mut(Spec::ALPHABET_SIZE).0.iter_mut() {
                let count = *start_freq;
                *start_freq = HistEnt::default().set_start(running_start as u16).set_freq(count as u16).into();
            }
        }
        CDF::<u32, AllocH, Spec>{
            sym:histogram.histogram,
            spec:spec,
            num_htrees:histogram.num_htrees
        }
    }
    pub fn new_from_group<AllocU32:Allocator<u32>, AllocHC:Allocator<HuffmanCode>>(alloc_u32: &mut AllocH, group:&HuffmanTreeGroup<AllocU32, AllocHC>, spec: Spec, mut old_ans: Option<Self>) -> Self {
        Self::new(alloc_u32, &group.htrees.slice()[..group.num_htrees as usize], group.codes.slice(), spec, old_ans)
    }
    pub fn free(&mut self, mh: &mut AllocH) {
        mh.free_cell(core::mem::replace(&mut self.sym, AllocH::AllocatedMemory::default()));
    }

}
pub struct ANSTable<HistEntTrait, Symbol:Sized+Ord+AddAssign<Symbol>+From<u8>+Clone, AllocS: Allocator<Symbol>, AllocH: Allocator<HistEntTrait>, Spec:HistogramSpec>
    where HistEnt:From<HistEntTrait> {
    state_lookup:AllocS::AllocatedMemory,
    cdf: CDF<HistEntTrait, AllocH, Spec>,
}
impl<Symbol:Sized+Ord+AddAssign<Symbol>+From<u8>+Clone,
     AllocS: Allocator<Symbol>,
     AllocH: Allocator<u32>, Spec:HistogramSpec> Default for ANSTable<u32, Symbol, AllocS, AllocH, Spec> {
    fn default() -> Self {
        ANSTable::<u32, Symbol, AllocS, AllocH, Spec> {
            state_lookup:AllocS::AllocatedMemory::default(),
            cdf:CDF::default(),
        }
    }
}
impl<Symbol:Sized+Ord+AddAssign<Symbol>+From<u8>+Clone+Copy,
     AllocS: Allocator<Symbol>,
     AllocH: Allocator<u32>, Spec:HistogramSpec> ANSTable<u32, Symbol, AllocS, AllocH, Spec> {
    pub fn new_single_code(alloc_u8: &mut AllocS, alloc_u32: &mut AllocH, group:&[HuffmanCode], spec: Spec, mut old_ans: Option<Self>) -> Self {
        Self::new(alloc_u8, alloc_u32, &[0], group, spec, old_ans)
    }
    pub fn new(alloc_u8: &mut AllocS, alloc_u32: &mut AllocH, group_count: &[u32], group:&[HuffmanCode], spec: Spec, mut old_ans: Option<Self>) -> Self {
        let (cdf, old_rev) = match old_ans {
            Some(old) => {
                (CDF::new(alloc_u32, group_count, group, spec, Some(old.cdf)), Some(old.state_lookup))
            }
            None => {
                (CDF::new(alloc_u32, group_count, group, spec, None), None)
            }
        };
        
        let mut rev = match old_rev {
            Some(old) => {
                if old.len() < group_count.len() as usize * TOT_FREQ as usize {
                    alloc_u8.free_cell(old);
                    alloc_u8.alloc_cell(group_count.len() as usize * TOT_FREQ as usize)
                } else {
                    old
                }
            },
            None => {
                alloc_u8.alloc_cell(group_count.len() as usize * TOT_FREQ as usize)
            }
        };
        for tree_id in 0..group_count.len() as usize{
            let mut sym = Symbol::from(0u8);
            let mut notfirst = 0u8;
            for start_freq in cdf.sym.slice().split_at(tree_id as usize * Spec::ALPHABET_SIZE).1.split_at(Spec::ALPHABET_SIZE).0 {
                sym += Symbol::from(notfirst);   
                let ent = HistEnt::from(*start_freq);
                let rev_slice = rev.slice_mut().split_at_mut(tree_id * TOT_FREQ as usize + ent.start() as usize).1;
                for rev_lk in rev_slice.split_at_mut(ent.freq() as usize).0 {
                    *rev_lk = sym;
                }
                notfirst = 1;
            }
        }
        ANSTable::<u32, Symbol, AllocS, AllocH, Spec>{
            state_lookup:rev,
            cdf:cdf,
        }
        
    }
    pub fn new_from_group<AllocU32:Allocator<u32>, AllocHC:Allocator<HuffmanCode>>(alloc_u8: &mut AllocS, alloc_u32: &mut AllocH, group:&HuffmanTreeGroup<AllocU32, AllocHC>, spec: Spec, mut old_ans: Option<Self>) -> Self {
        Self::new(alloc_u8, alloc_u32, &group.htrees.slice()[..group.num_htrees as usize], group.codes.slice(), spec, old_ans)
    }
    pub fn free(&mut self, ms: &mut AllocS, mh: &mut AllocH) {
        ms.free_cell(core::mem::replace(&mut self.state_lookup, AllocS::AllocatedMemory::default()));
        self.cdf.free(mh);
    }
}
#[derive(Clone,Copy,Default)]
pub struct CodeLengthPrefixSpec{}
impl HistogramSpec for CodeLengthPrefixSpec {
    const ALPHABET_SIZE: usize = 6;
}

#[derive(Clone,Copy,Default)]
pub struct LiteralSpec{}
impl HistogramSpec for LiteralSpec {
    const ALPHABET_SIZE: usize = 256;
}
pub type ContextMapSpec = LiteralSpec;
#[derive(Clone,Copy,Default)]
pub struct BlockLenSpec{}
impl HistogramSpec for BlockLenSpec {
    const ALPHABET_SIZE: usize = 26;
}

#[derive(Clone,Copy,Default)]
pub struct BlockTypeSpec{}
impl HistogramSpec for BlockTypeSpec {
    const ALPHABET_SIZE: usize = 258;
}

#[derive(Clone,Copy,Default)]
pub struct DistanceSpec{}
impl HistogramSpec for DistanceSpec {
    const ALPHABET_SIZE: usize = 704;
}

#[derive(Clone,Copy,Default)]
pub struct BlockLengthSpec{}
impl HistogramSpec for BlockLengthSpec {
    const ALPHABET_SIZE: usize = 26;
}
#[derive(Clone,Copy,Default)]
pub struct InsertCopySpec{}
impl HistogramSpec for InsertCopySpec {
    const ALPHABET_SIZE: usize = 704;
}

#[derive(Clone,Copy,Default)]
pub struct CodeLengthSymbolSpec {}

impl HistogramSpec for CodeLengthSymbolSpec {
    const ALPHABET_SIZE: usize = 18;
}

struct LiteralANSTable<AllocSym:Allocator<u8>, AllocH:Allocator<u32>>(ANSTable<u32, u8, AllocSym,  AllocH, LiteralSpec>);

struct DistanceANSTable<AllocSym:Allocator<u16>, AllocH:Allocator<u32>>(ANSTable<u32, u16, AllocSym,  AllocH, DistanceSpec>);
struct InsertCopyANSTable<AllocSym:Allocator<u16>, AllocH:Allocator<u32>>(ANSTable<u32, u16, AllocSym,  AllocH, InsertCopySpec>);

