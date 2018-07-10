use core::ops::AddAssign;
use core::cmp::Ord;
use core::convert::From;
use alloc;
use alloc::Allocator;
use alloc::SliceWrapper;
use alloc::SliceWrapperMut;
use super::HuffmanCode;
use super::HuffmanTreeGroup;
type Freq = u16;
const TF_SHIFT: Freq = 12;
const TOT_FREQ: Freq = 1 << TF_SHIFT;

pub struct HistEnt {
    pub start: Freq,
    pub freq: Freq,
}

trait HistogramSpec:Default {
    const ALPHABET_SIZE:usize;
    const MAX_SYMBOL:u64;
    
}

struct Histogram<AllocU32:Allocator<u32>, Spec:HistogramSpec> {
    histogram: AllocU32::AllocatedMemory,
    num_htrees: u16,
    spec:Spec,
}
impl<AllocU32:Allocator<u32>, Spec:HistogramSpec> Histogram<AllocU32, Spec> {
    fn new<AllocHC:Allocator<HuffmanCode>>(alloc_u32: &mut AllocU32,  group:&HuffmanTreeGroup<AllocU32,AllocHC>) -> Self{
        let mut  ret = Histogram::<AllocU32, Spec> {
            num_htrees:group.num_htrees,
            histogram:alloc_u32.alloc_cell(Spec::ALPHABET_SIZE * group.num_htrees as usize),
            spec:Spec::default(),
        };
        for cur_htree in 0..group.num_htrees {
            // lets collect samples from each htree
            let start = group.htrees.slice()[cur_htree as usize] as usize;
            let end = if cur_htree as usize + 1 < group.htrees.len() {
                group.htrees.slice()[cur_htree as usize + 1] as usize
            } else {
                group.codes.len()
            };
            for (index, code) in group.codes.slice()[start..end].iter().enumerate() {
                let count = (index < 256) as u32 * 255 + 1;
                let sym = code.value;
                if u64::from(code.value) <= Spec::MAX_SYMBOL {
                    ret.histogram.slice_mut()[cur_htree as usize * Spec::ALPHABET_SIZE + sym as usize] +=  count;
                }
            }
        }
        ret
    }
}
struct ANSTable<Symbol:Sized+Ord+AddAssign<Symbol>, AllocS: Allocator<Symbol>, AllocH: Allocator<HistEnt>, Spec:HistogramSpec>
    where u64:From<Symbol> {
    state_lookup:AllocS::AllocatedMemory,
    sym:AllocH::AllocatedMemory,
    spec: Spec,
    num_htrees: u16,
}
#[derive(Clone,Copy,Default)]
struct LiteralSpec{}
impl HistogramSpec for LiteralSpec {
    const ALPHABET_SIZE: usize = 256;
    const MAX_SYMBOL: u64 = 0xff;
}
#[derive(Clone,Copy,Default)]
struct DistanceSpec{}
impl HistogramSpec for DistanceSpec {
    const ALPHABET_SIZE: usize = 704;
    const MAX_SYMBOL: u64 = 703;
}
#[derive(Clone,Copy,Default)]
struct BlockLengthSpec{}
impl HistogramSpec for BlockLengthSpec {
    const ALPHABET_SIZE: usize = 26;
    const MAX_SYMBOL: u64 = 25;
}
struct LiteralANSTable<AllocSym:Allocator<u8>, AllocH:Allocator<HistEnt>>(ANSTable<u8, AllocSym,  AllocH, LiteralSpec>);

struct DistanceANSTable<AllocSym:Allocator<u16>, AllocH:Allocator<HistEnt>>(ANSTable<u16, AllocSym,  AllocH, DistanceSpec>);
