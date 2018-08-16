use super::super::bit_reader;
use ::BrotliResult;
use core::marker::PhantomData;
use alloc;
use alloc::Allocator;
use super::super::{HuffmanCode, HuffmanTreeGroup};
use super::super::huffman::histogram::{ANSTable, HistogramSpec, HistEnt, TF_SHIFT, FrequentistCDF};
use super::interface::*;
use core::ops::AddAssign;
use core::cmp::max;
use entropy::log4096::LOG4096;
use probability::interface::{CDF16,CDF_MAX, LOG2_SCALE, Speed, BaseCDF, BLEND_FIXED_POINT_PRECISION};
use probability::frequentist_cdf::{FrequentistCDF16};
use std::vec;
type CDF = FrequentistCDF16;
use probability::weights::Weights;

#[derive(Clone, Copy)]
struct LiteCDF {
    samples: [i32;256],
    count: usize,
}
impl Default for LiteCDF {
    fn default() -> Self {
        LiteCDF {
            samples:[0i32;256],
            count:0,
        }
    }
}
pub struct BillingEncoder {
    ucdf: vec::Vec<CDF>,
    lcdf: vec::Vec<CDF>,
    total: [f64; 6],
    spec: [f64;6],
    weights: [Weights;2],
    adaptive: vec::Vec<LiteCDF>,
    adaptive_snap: vec::Vec<LiteCDF>,
    semi_adaptive: vec::Vec<LiteCDF>,
    semi_adaptive_snap: vec::Vec<LiteCDF>,
}

impl Default for BillingEncoder {
    fn default() -> Self {
        BillingEncoder {
            ucdf:vec![CDF::default(); 65536],
            lcdf:vec![CDF::default(); 65536],
            total:[0.0;6],
            spec:[0.0;6],
            weights:[Weights::new(), Weights::new()],
            adaptive:vec::Vec::new(),
            adaptive_snap:vec::Vec::new(),
            semi_adaptive:vec![LiteCDF{samples:[1i32;256],count:256};65536],
            semi_adaptive_snap:vec![LiteCDF{samples:[1i32;256],count:256};65536],
        }
    }
}

fn approx_freq(denom: HistEnt, num: HistEnt) -> usize {
    let num_u16 = num.freq();
    let denom_u16 = denom.freq();
    if denom_u16 == num_u16 {
        return 1 << TF_SHIFT;
    }
    let num_u32 = u32::from(num_u16) << TF_SHIFT;
    let mut ret = ((num_u32 + (u32::from(denom_u16) - 1))/u32::from(denom_u16));
    if ret > 1 {
        ret -= 1;
    }
    assert!(ret * u32::from(denom_u16) < num_u32);
    assert!((1 + ret) * u32::from(denom_u16) >= num_u32);
    ret as usize
}
#[no_mangle]
fn dosomething() {
    eprintln!("SOMETHING");
}
#[allow(unused)]
impl<AllocU8:Allocator<u8>,AllocU32: Allocator<u32>> EntropyEncoder<AllocU8, AllocU32> for BillingEncoder {
    fn put<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocCDF : Allocator<FrequentistCDF>, Spec:HistogramSpec, Speculative:BoolTrait> (
        &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        group:&[&[HuffmanCode];256],
        prob: &ANSTable<u32, Symbol, AllocS, AllocH, AllocCDF, Spec>,
        prior: (u8, u8, u8),
        mut symbol: Symbol,
        is_speculative: Speculative,
    ){
        let mut b16hist_ent = prob.get_prob(prior.0, (symbol.into_u64() as u32) & 0xfff0);
        for index in 1..16 {
            let mut cur_hist_ent = prob.get_prob(prior.0, ((symbol.into_u64() as u32) & 0xfff0) | index);
            let old = b16hist_ent.freq();
            b16hist_ent.set_freq(cur_hist_ent.freq() + old);
        }
        let mut hist_ent = prob.get_prob(prior.0, symbol.into_u64() as u32);
        assert!(hist_ent.freq() != 0);
        let val = LOG4096[hist_ent.freq() as usize];
        let ufreq = b16hist_ent.freq();
        let lfreq = approx_freq(b16hist_ent, hist_ent);
        let (val_unib, val_lnib) = if Spec::ALPHABET_SIZE == 256 {
          (LOG4096[ufreq as usize], LOG4096[lfreq])
        } else {
          (val, 0.0)
        };
        
        let (cdf_val_unib, cdf_val_lnib) = if Spec::ALPHABET_SIZE == 256 {
            let upper_nibble = (symbol.into_u64() as usize & 0xf0) >> 4;
            let lower_nibble = (symbol.into_u64() as usize & 0xf);
            let hcdf = prob.nibble.high_nibble(usize::from(prior.0));
            let lcdf = prob.nibble.low_nibble(usize::from(prior.0))[upper_nibble];
            
            ((hcdf.pdf(upper_nibble as u8) as f64 / hcdf.max() as f64).log2(),
             (lcdf.pdf(lower_nibble as u8) as f64 / lcdf.max() as f64).log2())
        } else {
            (val, 0.0)
        };
        if Spec::ALPHABET_SIZE == 256 {
            if self.adaptive.len() == 0 {
                self.adaptive = vec![LiteCDF::default();65536];
                for prior in 0..prob.num_htrees() as usize {
                    for prev_byte in 0..256 {
                        let sub_cdf = &mut self.adaptive[(prior << 8) | prev_byte];
                        let count = prob.copy_freq(&mut sub_cdf.samples, prior as u8);
                        sub_cdf.count = count;
                    }
                }
                self.adaptive_snap = self.adaptive.clone();
            } // try occasionally-updated stats
            let prior_index = (usize::from(prior.0) << 8) | usize::from(prior.1);
            let inc = 16;
            let adaptive_prob = self.adaptive_snap[prior_index].samples[symbol.into_u64() as usize] as f64 /self.adaptive_snap[prior_index].count as f64;
            self.adaptive[prior_index].samples[symbol.into_u64() as usize] += inc;
            self.adaptive[prior_index].count += inc as usize;
            let semi_count_thresh = 8192;
            let semi_adaptive_prob = if self.semi_adaptive_snap[prior_index].count >= semi_count_thresh {
                self.semi_adaptive_snap[prior_index].samples[symbol.into_u64() as usize] as f64 /self.semi_adaptive_snap[prior_index].count as f64
            } else {
                hist_ent.freq() as f64 / 4096.0
            };

            self.semi_adaptive[prior_index].samples[symbol.into_u64() as usize] += inc;
            self.semi_adaptive[prior_index].count += inc as usize;
            let snap_period = 64;
            if self.semi_adaptive[prior_index].count % snap_period == 0 {
                self.semi_adaptive_snap[prior_index] = self.semi_adaptive[prior_index];
            }
            if self.adaptive[prior_index].count % snap_period == 0 {
                self.adaptive_snap[prior_index] = self.adaptive[prior_index];
            }
            if Speculative::VALUE {
                self.spec[4] -= adaptive_prob.log2();
                self.spec[5] -= semi_adaptive_prob.log2();
            } else {
                self.total[4] -= adaptive_prob.log2();
                self.total[5] -= semi_adaptive_prob.log2();
            }
        }

        if Spec::ALPHABET_SIZE == 256 {
            let upper_nibble = (symbol.into_u64() as usize & 0xf0) >> 4;
            let lower_nibble = (symbol.into_u64() as usize & 0xf);
            let hcdf = prob.nibble.high_nibble(usize::from(prior.0));
            let lcdf = prob.nibble.low_nibble(usize::from(prior.0))[upper_nibble];
            let h_baseline_freq = hcdf.sym_to_start_and_freq(upper_nibble as u8);
            let l_baseline_freq = lcdf.sym_to_start_and_freq(lower_nibble as u8);
            let primary_index = usize::from(prior.0) + usize::from(prior.1) * 256;
            let u_est_freq = self.ucdf[primary_index].sym_to_start_and_freq(upper_nibble as u8);
            let secondary_index = usize::from(prior.1) + ((symbol.into_u64() as usize &0xfff0) << 4);
            let l_est_freq = self.ucdf[secondary_index].sym_to_start_and_freq(lower_nibble as u8 & 0xf);
            let upper_avg = self.ucdf[primary_index].average(&hcdf, self.weights[0].norm_weight().into());
            let lower_avg = self.ucdf[secondary_index].average(&lcdf, self.weights[1].norm_weight().into());
            let u_blend_freq = upper_avg.sym_to_start_and_freq(upper_nibble as u8);
            let l_blend_freq = lower_avg.sym_to_start_and_freq(lower_nibble as u8);            
            self.ucdf[primary_index].blend((symbol.into_u64() >> 4) as u8, Speed::new(32,4096));
            self.ucdf[secondary_index].blend(symbol.into_u64() as u8 & 0xf, Speed::new(32,4096));
            let u_entropy = (u_est_freq.range.freq as  f64 / (CDF_MAX as f64)).log2();
            let l_entropy = (l_est_freq.range.freq as  f64 / (CDF_MAX as f64)).log2();
            let u_blend_entropy = (u_blend_freq.range.freq as  f64 / (CDF_MAX as f64)).log2();
            let l_blend_entropy = (l_blend_freq.range.freq as  f64 / (CDF_MAX as f64)).log2();
            if !(u_blend_entropy == u_blend_entropy) {
                dosomething();
                let upper_avg2 = self.ucdf[primary_index].average(&hcdf, self.weights[0].norm_weight().into());
            }
            if !(l_blend_entropy == l_blend_entropy) {
                dosomething();
                let lower_avg2 = self.ucdf[secondary_index].average(&lcdf, self.weights[1].norm_weight().into());
            }
            self.weights[0].update([u_est_freq.range.freq,max(1,h_baseline_freq.range.freq)], max(u_blend_freq.range.freq,1));
            self.weights[1].update([l_est_freq.range.freq,max(1,l_baseline_freq.range.freq)], max(l_blend_freq.range.freq,1));
            if Speculative::VALUE {
                self.spec[1] -= u_entropy + l_entropy;
                self.spec[2] -= u_blend_entropy + l_blend_entropy;
            } else {
                self.total[1] -= u_entropy + l_entropy;
                self.total[2] -= u_blend_entropy + l_blend_entropy;
            }
        } else {
          if Speculative::VALUE {
              self.spec[1] -= val_unib;
              self.spec[1] -= val_lnib;
              self.spec[2] -= val;
              self.spec[4] -= val;
              self.spec[5] -= val;
          } else {
              self.total[1] -= val_unib;
              self.total[1] -= val_lnib;
              self.total[2] -= val;
              self.total[4] -= val;
              self.total[5] -= val;
          }
        }
        if Speculative::VALUE {
            self.spec[0] -= val;
            self.spec[3] -= cdf_val_unib;
            self.spec[3] -= cdf_val_lnib;
        } else {
            self.total[0] -= val;
            self.total[3] -= cdf_val_unib;
            self.total[3] -= cdf_val_lnib;
        }
        
    }
    fn put_stationary<Symbol: Sized+Ord+AddAssign<Symbol>+From<u8>+SymbolCast + Clone, AllocS:Allocator<Symbol>, AllocH: Allocator<u32>, AllocCDF : Allocator<FrequentistCDF>, Spec:HistogramSpec, Speculative:BoolTrait>(
        &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        group:&[HuffmanCode],
        prob: &ANSTable<u32, Symbol, AllocS, AllocH, AllocCDF, Spec>,
        l1numbits: u8,
        symbol: Symbol, 
        speculative: Speculative) {
        let mut hist_ent = prob.get_prob(0, symbol.into_u64() as u32);
        assert!(hist_ent.freq() != 0);
        let val = LOG4096[hist_ent.freq() as usize];
        for index in 0..self.total.len() {
            if Speculative::VALUE {
              self.spec[index] -= val;
            } else {
              self.total[index] -= val;
            }
        }
    }
    fn put_uniform<Speculative:BoolTrait> (
        &mut self,
        m8: &mut AllocU8, m32: &mut AllocU32,
        nbits: u8,
        symbol: u32,
        is_speculative: Speculative) {
        for index in 0..self.total.len() {
            if Speculative::VALUE {
                self.spec[index] += nbits as f64;
            } else {
                self.total[index] += nbits as f64;
            }    
        }
    }
    fn begin_speculative(&mut self){}
    fn commit_speculative(&mut self){
        for index in 0..self.spec.len() {
            self.total[index] += self.spec[index];
            self.spec[index] = 0.0;
        }
    }
    fn abort_speculative(&mut self){
        for item in self.spec.iter_mut() {
            *item = 0.0;
        }
  }
  fn drain(&mut self, out_data: &mut [u8]) -> usize {0}
  fn finish(&mut self, out_data:&mut [u8]) -> usize {
      eprintln!("Total: {} bits, {} bytes\nAdapt: {} bits, {} bytes", self.total[0], self.total[0] / 8.0,
                self.total[1], self.total[1] / 8.0);
      eprintln!("Mixin: {} bits, {} bytes\nCDFMixing {} {}", self.total[2], self.total[2] / 8.0,
                self.total[3], self.total[3] / 8.0);
      eprintln!("Snap0: {} bits, {} bytes\nCDFSemiSnap {} {}", self.total[4], self.total[4] / 8.0,
                self.total[5], self.total[5] / 8.0);
    0
  }
}

