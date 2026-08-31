#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
mod tests;
use ::core;
use alloc;
use alloc::Allocator;
use alloc::SliceWrapper;
use alloc::SliceWrapperMut;
use core::default::Default;
pub const BROTLI_HUFFMAN_MAX_CODE_LENGTH: usize = 15;

// For current format this constant equals to kNumInsertAndCopyCodes
pub const BROTLI_HUFFMAN_MAX_CODE_LENGTHS_SIZE: usize = 704;

// Maximum possible Huffman table size for an alphabet size of (index * 32),
// max code length 15 and root table bits 8.
// pub const kMaxHuffmanTableSize : [u16;23] = [
// 256, 402, 436, 468, 500, 534, 566, 598, 630, 662, 694, 726, 758, 790, 822,
// 854, 886, 920, 952, 984, 1016, 1048, 1080, 1112, 1144,1176,1208,1240,272,
// 1304, 1336, 1368, 1400, 1432, 1464, 1496, 1528];
// pub const BROTLI_HUFFMAN_MAX_SIZE_26 : u32 = 396;
// pub const BROTLI_HUFFMAN_MAX_SIZE_258 : u32 = 632;
// pub const BROTLI_HUFFMAN_MAX_SIZE_272 : u32 = 646;
//
pub const BROTLI_HUFFMAN_MAX_TABLE_SIZE: u32 = 1080;
pub const BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH: u32 = 5;

#[repr(C)]
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct HuffmanCode {
  pub value: u16, // symbol value or table offset
  pub bits: u8, // number of bits used for this symbol
}

impl HuffmanCode {
  pub fn eq(&self, other: &Self) -> bool {
    self.value == other.value && self.bits == other.bits
  }
}

impl Default for HuffmanCode {
  fn default() -> Self {
    HuffmanCode {
      value: 0,
      bits: 0,
    }
  }
}

// Contains a collection of Huffman trees with the same alphabet size.
pub struct HuffmanTreeGroup<Alloc32: Allocator<u32>, AllocHC: Allocator<HuffmanCode>> {
  pub htrees: Alloc32::AllocatedMemory,
  pub codes: AllocHC::AllocatedMemory,
  pub alphabet_size: u16,
  pub max_symbol: u16,
  pub num_htrees: u16,
}

impl<AllocU32 : alloc::Allocator<u32>,
     AllocHC : alloc::Allocator<HuffmanCode> > HuffmanTreeGroup<AllocU32, AllocHC> {
    pub fn init(self : &mut Self, mut alloc_u32 : &mut AllocU32, mut alloc_hc : &mut AllocHC,
                alphabet_size : u16, max_symbol: u16, ntrees : u16) {
        self.reset(&mut alloc_u32, &mut alloc_hc);
        self.alphabet_size = alphabet_size;
        self.max_symbol = max_symbol;
        self.num_htrees = ntrees;
        let nt = ntrees as usize;
        let _ = core::mem::replace(&mut self.htrees,
                           alloc_u32.alloc_cell(nt));
        let _ = core::mem::replace(&mut self.codes,
                           alloc_hc.alloc_cell(nt * BROTLI_HUFFMAN_MAX_TABLE_SIZE as usize));
    }

//  pub fn get_tree_mut<'a>(self :&'a mut Self, index : u32, mut tree_out : &'a mut [HuffmanCode]) {
//        let start : usize = fast!((self.htrees)[index as usize]) as usize;
//        let _ = core::mem::replace(&mut tree_out, fast_mut!((self.codes.slice_mut())[start;]));
//    }
//    pub fn get_tree<'a>(self :&'a Self, index : u32, mut tree_out : &'a [HuffmanCode]) {
//        let start : usize = fast!((self.htrees)[index as usize]) as usize;
//        let _ = core::mem::replace(&mut tree_out, fast_slice!((self.codes)[start;]));
//    }
    #[allow(dead_code)]
    pub fn get_tree_mut(&mut self, index : u32) -> &mut [HuffmanCode] {
        let start : usize = fast_slice!((self.htrees)[index as usize]) as usize;
        fast_mut!((self.codes.slice_mut())[start;])
    }
    #[allow(dead_code)]
    pub fn get_tree(&self, index : u32) -> &[HuffmanCode] {
        let start : usize = fast_slice!((self.htrees)[index as usize]) as usize;
        fast_slice!((self.codes)[start;])
    }
    pub fn reset(self : &mut Self, alloc_u32 : &mut AllocU32, alloc_hc : &mut AllocHC) {
        alloc_u32.free_cell(core::mem::replace(&mut self.htrees,
                                               AllocU32::AllocatedMemory::default()));
        alloc_hc.free_cell(core::mem::replace(&mut self.codes,
                                              AllocHC::AllocatedMemory::default()));

// for mut iter in self.htrees[0..self.num_htrees as usize].iter_mut() {
//    if iter.slice().len() > 0 {
//        alloc_hc.free_cell(core::mem::replace(&mut iter,
//                                              AllocHC::AllocatedMemory::default()));
//    }
// }

    }
    pub fn build_hgroup_cache(&self) -> [&[HuffmanCode]; 256] {
      let mut ret : [&[HuffmanCode]; 256] = [&[]; 256];
      let mut index : usize = 0;
      for htree in self.htrees.slice() {
          ret[index] = fast_slice!((&self.codes)[*htree as usize ; ]);
          index += 1;
      }
      ret
    }
}

impl<AllocU32 : alloc::Allocator<u32>,
     AllocHC : alloc::Allocator<HuffmanCode> > Default for HuffmanTreeGroup<AllocU32, AllocHC> {
    fn default() -> Self {
        HuffmanTreeGroup::<AllocU32, AllocHC> {
          htrees : AllocU32::AllocatedMemory::default(),
          codes : AllocHC::AllocatedMemory::default(),
          max_symbol: 0,
          alphabet_size : 0,
          num_htrees : 0,
        }
    }
}



const BROTLI_REVERSE_BITS_MAX: usize = 8;

const BROTLI_REVERSE_BITS_BASE: u8 = 0;

const BROTLI_REVERSE_BITS_LOWEST: u32 =
  (1u32 << (BROTLI_REVERSE_BITS_MAX as u32 - 1 + BROTLI_REVERSE_BITS_BASE as u32));

// Callers narrow their key with `as u8`, which is sound only because this port
// uses BROTLI_REVERSE_BITS_BASE == 0: keys accumulate from
// BROTLI_REVERSE_BITS_LOWEST and so stay under
// 1 << (BROTLI_REVERSE_BITS_MAX + BROTLI_REVERSE_BITS_BASE).
//
// That is not a spec guarantee. The reference implementation sets
// BROTLI_REVERSE_BITS_BASE = (sizeof(brotli_reg_t) << 3) - BROTLI_REVERSE_BITS_MAX
// (56 on 64-bit) wherever BROTLI_RBIT is available, which puts the key in the
// high bits of a full register and makes it far larger than a u8. If that is
// ever mirrored here the narrowing below would silently truncate, so fail the
// build instead.
const _REVERSE_BITS_KEYS_FIT_IN_U8: [(); 256] =
  [(); 1usize << (BROTLI_REVERSE_BITS_MAX + BROTLI_REVERSE_BITS_BASE as usize)];

// Returns reverse(num >> BROTLI_REVERSE_BITS_BASE, BROTLI_REVERSE_BITS_MAX),
// where reverse(value, len) is the bit-wise reversal of the len least
// significant bits of value.
//
// Callers narrow to u8, which is sound because BrotliBuildHuffmanTable rejects
// root_bits outside [1, BROTLI_REVERSE_BITS_MAX] up front and
// BrotliBuildCodeLengthsHuffmanTable requires
// BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH <= BROTLI_REVERSE_BITS_MAX, which
// bounds every key by 1 << 8.
fn BrotliReverseBits(num: u8) -> u32 {
  num.reverse_bits() as u32
}

// Stores code in table[0], table[step], table[2*step], ..., table[end]
// Assumes that end is an integer multiple of step
fn ReplicateValue(table: &mut [HuffmanCode],
                  offset: u32,
                  step: i32,
                  mut end: i32,
                  code: HuffmanCode) -> bool {
  if step <= 0 || end <= 0 || end % step != 0 {
    return false;
  }
  loop {
    end -= step;
    let index = match (offset as usize).checked_add(end as usize) {
      Some(index) => index,
      None => return false,
    };
    match table.get_mut(index) {
      Some(value) => *value = code,
      None => return false,
    }
    if end == 0 {
      break;
    }
  }
  true
}

// Returns the table width of the next 2nd level table. count is the histogram
// of bit lengths for the remaining symbols, len is the code length of the next
// processed symbol
fn NextTableBitSize(count: &[u16], mut len: i32, root_bits: i32) -> Option<i32> {
  let shift = match len.checked_sub(root_bits) {
    Some(shift) if shift >= 0 => shift as u32,
    _ => return None,
  };
  let mut left = match 1i32.checked_shl(shift) {
    Some(left) => left,
    None => return None,
  };
  while len < BROTLI_HUFFMAN_MAX_CODE_LENGTH as i32 {
    let count_value = match count.get(len as usize) {
      Some(value) => *value as i32,
      None => return None,
    };
    left = match left.checked_sub(count_value) {
      Some(left) => left,
      None => return None,
    };
    if left <= 0 {
      break;
    }
    len += 1;
    left = match left.checked_shl(1) {
      Some(left) => left,
      None => return None,
    };
  }
  len.checked_sub(root_bits)
}

fn symbol_list_value(symbol_lists: &[u16],
                     symbol_lists_offset: usize,
                     relative_index: i32) -> Option<u16> {
  let index = if relative_index < 0 {
    let distance = match relative_index.checked_neg() {
      Some(distance) => distance as usize,
      None => return None,
    };
    match symbol_lists_offset.checked_sub(distance) {
      Some(index) => index,
      None => return None,
    }
  } else {
    match symbol_lists_offset.checked_add(relative_index as usize) {
      Some(index) => index,
      None => return None,
    }
  };
  match symbol_lists.get(index) {
    Some(value) => Some(*value),
    None => None,
  }
}


pub fn BrotliBuildCodeLengthsHuffmanTable(mut table: &mut [HuffmanCode],
                                          code_lengths: &[u8],
                                          count: &[u16]) -> bool {
  let mut sorted: [i32; 18] = fast_uninitialized![18];     /* symbols sorted by code length */
  // offsets in sorted table for each length
  let mut offset: [i32; (BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH + 1) as usize] =
    fast_uninitialized![(BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH + 1) as usize];
  const table_size: i32 = 1 << BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH;
  if BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH as usize > BROTLI_REVERSE_BITS_MAX ||
     table.len() < table_size as usize ||
     code_lengths.len() < sorted.len() ||
     count.len() <= BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH as usize {
    return false;
  }
  let mut actual_count =
    [0u16; (BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH + 1) as usize];
  for code_length in code_lengths.iter().take(sorted.len()) {
    let code_length_index = *code_length as usize;
    if code_length_index >= actual_count.len() {
      return false;
    }
    actual_count[code_length_index] += 1;
  }
  if actual_count[1..] != count[1..actual_count.len()] {
    return false;
  }

  // generate offsets into sorted symbol table by code length
  let mut symbol: i32 = -1;         /* symbol index in original or sorted table */
  let mut bits: i32 = 1;
  for _ in 0..BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH {
    symbol += fast!((count)[bits as usize]) as i32;
    fast_mut!((offset)[bits as usize]) = symbol;
    bits += 1;
  }
  // Symbols with code length 0 are placed after all other symbols.
  fast_mut!((offset)[0]) = 17;

  // sort symbols by length, by symbol order within each length
  symbol = 18;
  loop {
    for _ in 0..6 {
      symbol -= 1;
      let index = fast!((offset)[fast_inner!((code_lengths)[symbol as usize]) as usize]);
      fast_mut!((offset)[fast_inner!((code_lengths)[symbol as usize]) as usize]) -= 1;
      fast_mut!((sorted)[index as usize]) = symbol;
    }
    if symbol == 0 {
      break;
    }
  }

  // Special case: all symbols but one have 0 code length.
  if fast!((offset)[0]) == 0 {
    let code: HuffmanCode = HuffmanCode {
      bits: 0,
      value: fast!((sorted)[0]) as u16,
    };
    for val in fast_mut!((table)[0 ; table_size as usize]).iter_mut() {
      *val = code;
    }
    return true;
  }

  // fill in table
  let mut key: u32 = 0; /* prefix code */
  let mut key_step: u32 = BROTLI_REVERSE_BITS_LOWEST; /* prefix code addend */
  symbol = 0;
  bits = 1;
  let mut step: i32 = 2;
  loop {
    let mut code: HuffmanCode = HuffmanCode {
      bits: (bits as u8),
      value: 0,
    };
    let mut bits_count: i32 = fast!((count)[bits as usize]) as i32;

    while bits_count != 0 {
      code.value = fast!((sorted)[symbol as usize]) as u16;
      symbol += 1;
      let reversed_key = BrotliReverseBits(key as u8);
      if !ReplicateValue(&mut table, reversed_key, step, table_size, code) {
        return false;
      }
      key = match key.checked_add(key_step) {
        Some(key) => key,
        None => return false,
      };
      bits_count -= 1;
    }
    step <<= 1;
    key_step >>= 1;
    bits += 1;
    if !(bits <= BROTLI_HUFFMAN_MAX_CODE_LENGTH_CODE_LENGTH as i32) {
      break;
    }
  }
  true
}

pub fn BrotliBuildHuffmanTable(mut root_table: &mut [HuffmanCode],
                               root_bits: i32,
                               symbol_lists: &[u16],
                               symbol_lists_offset: usize, /* need negative-index to symbol_lists */
                               count: &mut [u16])
                               -> u32 {
  let mut code: HuffmanCode = HuffmanCode {
    bits: 0,
    value: 0,
  };       /* current table entry */
  let mut max_length: i32 = -1;

  if root_bits <= 0 ||
     root_bits as usize > BROTLI_REVERSE_BITS_MAX ||
     BROTLI_HUFFMAN_MAX_CODE_LENGTH as i32 - root_bits >
       BROTLI_REVERSE_BITS_MAX as i32 ||
     symbol_lists_offset >= symbol_lists.len() {
    return 0;
  }

  while match symbol_list_value(symbol_lists, symbol_lists_offset, max_length) {
    Some(value) => value == 0xFFFF,
    None => return 0,
  } {
    max_length -= 1;
  }
  max_length += BROTLI_HUFFMAN_MAX_CODE_LENGTH as i32 + 1;
  if max_length < 0 || max_length as usize >= count.len() {
    return 0;
  }

  let mut table_free_offset: u32 = 0;
  let mut table_bits: i32 = root_bits;      /* key length of current table */
  let mut table_size = match 1i32.checked_shl(table_bits as u32) {
    Some(table_size) => table_size,
    None => return 0,
  };                                      /* size of current table */
  let mut total_size: i32 = table_size;     /* sum of root table size and 2nd level table sizes */

  // fill in root table
  // let's reduce the table size to a smaller size if possible, and
  // create the repetitions by memcpy if possible in the coming loop
  if table_bits > max_length {
    table_bits = max_length;
    table_size = match 1i32.checked_shl(table_bits as u32) {
      Some(table_size) => table_size,
      None => return 0,
    };
  }
  let mut key: u32 = 0; /* prefix code */
  let mut key_step: u32 = BROTLI_REVERSE_BITS_LOWEST; /* prefix code addend */
  let mut bits: i32 = 1;
  let mut step: i32 = 2; /* step size to replicate values in current table */
  loop {
    code.bits = bits as u8;
    let mut symbol: i32 = bits - (BROTLI_HUFFMAN_MAX_CODE_LENGTH as i32 + 1);
    let mut bits_count: i32 = fast!((count)[bits as usize]) as i32;
    while bits_count != 0 {
      symbol = match symbol_list_value(symbol_lists, symbol_lists_offset, symbol) {
        Some(symbol) => symbol as i32,
        None => return 0,
      };
      code.value = symbol as u16;
      let reversed_key = BrotliReverseBits(key as u8);
      let table_offset = match table_free_offset.checked_add(reversed_key) {
        Some(table_offset) => table_offset,
        None => return 0,
      };
      if !ReplicateValue(&mut root_table, table_offset, step, table_size, code) {
        return 0;
      }
      key = match key.checked_add(key_step) {
        Some(key) => key,
        None => return 0,
      };
      bits_count -= 1;
    }
    step <<= 1;
    key_step >>= 1;
    bits += 1;
    if !(bits <= table_bits) {
      break;
    }
  }

  // if root_bits != table_bits we only created one fraction of the
  // table, and we need to replicate it now.
  while total_size != table_size {
    if table_size <= 0 || table_size > total_size {
      return 0;
    }
    for index in 0..table_size {
      let source_index = match (table_free_offset as usize).checked_add(index as usize) {
        Some(source_index) => source_index,
        None => return 0,
      };
      let destination_index =
        match source_index.checked_add(table_size as usize) {
          Some(destination_index) => destination_index,
          None => return 0,
        };
      let value = match root_table.get(source_index) {
        Some(value) => *value,
        None => return 0,
      };
      match root_table.get_mut(destination_index) {
        Some(destination) => *destination = value,
        None => return 0,
      }
    }
    table_size = match table_size.checked_shl(1) {
      Some(table_size) => table_size,
      None => return 0,
    };
  }

  // fill in 2nd level tables and add pointers to root table
  key_step = BROTLI_REVERSE_BITS_LOWEST >> (root_bits - 1);
  let mut sub_key: u32 = BROTLI_REVERSE_BITS_LOWEST << 1;       /* 2nd level table prefix code */
  let mut sub_key_step: u32 = BROTLI_REVERSE_BITS_LOWEST;   /* 2nd level table prefix code addend */

  step = 2;

  let mut len: i32 = root_bits + 1; /* current code length */
  while len <= max_length {
    let mut symbol: i32 = len - (BROTLI_HUFFMAN_MAX_CODE_LENGTH as i32 + 1);
    while fast!((count)[len as usize]) != 0 {
      if sub_key == (BROTLI_REVERSE_BITS_LOWEST << 1u32) {
        table_free_offset = match table_free_offset.checked_add(table_size as u32) {
          Some(table_free_offset) => table_free_offset,
          None => return 0,
        };
        table_bits = match NextTableBitSize(count, len, root_bits) {
          Some(table_bits) => table_bits,
          None => return 0,
        };
        table_size = match 1i32.checked_shl(table_bits as u32) {
          Some(table_size) => table_size,
          None => return 0,
        };
        total_size = match total_size.checked_add(table_size) {
          Some(total_size) => total_size,
          None => return 0,
        };
        sub_key = BrotliReverseBits(key as u8);
        key = match key.checked_add(key_step) {
          Some(key) => key,
          None => return 0,
        };
        let table_value =
          match (table_free_offset as usize).checked_sub(sub_key as usize) {
            Some(table_value) if table_value <= u16::MAX as usize => table_value as u16,
            _ => return 0,
          };
        match root_table.get_mut(sub_key as usize) {
          Some(entry) => {
            entry.bits = (table_bits + root_bits) as u8;
            entry.value = table_value;
          },
          None => return 0,
        }
        sub_key = 0;
      }
      code.bits = (len - root_bits) as u8;
      symbol = match symbol_list_value(symbol_lists, symbol_lists_offset, symbol) {
        Some(symbol) => symbol as i32,
        None => return 0,
      };
      code.value = symbol as u16;
      let reversed_sub_key = BrotliReverseBits(sub_key as u8);
      let table_offset = match table_free_offset.checked_add(reversed_sub_key) {
        Some(table_offset) => table_offset,
        None => return 0,
      };
      if !ReplicateValue(&mut root_table, table_offset, step, table_size, code) {
        return 0;
      }
      sub_key = match sub_key.checked_add(sub_key_step) {
        Some(sub_key) => sub_key,
        None => return 0,
      };
      match count.get_mut(len as usize) {
        Some(count_value) if *count_value != 0 => *count_value -= 1,
        _ => return 0,
      }
    }
    step = match step.checked_shl(1) {
      Some(step) => step,
      None => return 0,
    };
    sub_key_step >>= 1;
    len += 1
  }
  total_size as u32
}



pub fn BrotliBuildSimpleHuffmanTable(table: &mut [HuffmanCode],
                                     root_bits: i32,
                                     val: &[u16],
                                     num_symbols: u32)
                                     -> u32 {
  if root_bits <= 0 || root_bits >= 32 || num_symbols > 4 {
    return 0;
  }
  let required_symbols = match num_symbols {
    0 => 1,
    1 => 2,
    2 | 3 => 3,
    4 => 4,
    _ => return 0,
  };
  if val.len() < required_symbols {
    return 0;
  }
  let mut table_size: u32 = 1;
  let goal_size = match 1u32.checked_shl(root_bits as u32) {
    Some(goal_size) => goal_size,
    None => return 0,
  };
  if table.len() < goal_size as usize {
    return 0;
  }
  if num_symbols == 0 {
    fast_mut!((table)[0]).bits = 0;
    fast_mut!((table)[0]).value = fast!((val)[0]);
  } else if num_symbols == 1 {
    fast_mut!((table)[0]).bits = 1;
    fast_mut!((table)[1]).bits = 1;
    if fast!((val)[1]) > fast!((val)[0]) {
      fast_mut!((table)[0]).value = fast!((val)[0]);
      fast_mut!((table)[1]).value = fast!((val)[1]);
    } else {
      fast_mut!((table)[0]).value = fast!((val)[1]);
      fast_mut!((table)[1]).value = fast!((val)[0]);
    }
    table_size = 2;
  } else if num_symbols == 2 {
    fast_mut!((table)[0]).bits = 1;
    fast_mut!((table)[0]).value = fast!((val)[0]);
    fast_mut!((table)[2]).bits = 1;
    fast_mut!((table)[2]).value = fast!((val)[0]);
    if fast!((val)[2]) > fast!((val)[1]) {
      fast_mut!((table)[1]).value = fast!((val)[1]);
      fast_mut!((table)[3]).value = fast!((val)[2]);
    } else {
      fast_mut!((table)[1]).value = fast!((val)[2]);
      fast_mut!((table)[3]).value = fast!((val)[1]);
    }
    fast_mut!((table)[1]).bits = 2;
    fast_mut!((table)[3]).bits = 2;
    table_size = 4;
  } else if num_symbols == 3 {
    let last: u16 = if val.len() > 3 { fast!((val)[3]) } else { 65535 };
    let mut mval: [u16; 4] = [fast!((val)[0]), fast!((val)[1]), fast!((val)[2]), last];
    for i in 0..3 {
      for k in i + 1..4 {
        if mval[k] < mval[i] {
          mval.swap(k, i);
        }
      }
    }
    for i in 0..4 {
      fast_mut!((table)[i]).bits = 2;
    }
    fast_mut!((table)[0]).value = mval[0];
    fast_mut!((table)[2]).value = mval[1];
    fast_mut!((table)[1]).value = mval[2];
    fast_mut!((table)[3]).value = mval[3];
    table_size = 4;
  } else if num_symbols == 4 {
    let mut mval: [u16; 4] = [fast!((val)[0]), fast!((val)[1]), fast!((val)[2]), fast!((val)[3])];
    if mval[3] < mval[2] {
      mval.swap(3, 2)
    }
    for i in 0..7 {
      fast_mut!((table)[i]).value = mval[0];
      fast_mut!((table)[i]).bits = (1 + (i & 1)) as u8;
    }
    fast_mut!((table)[1]).value = mval[1];
    fast_mut!((table)[3]).value = mval[2];
    fast_mut!((table)[5]).value = mval[1];
    fast_mut!((table)[7]).value = mval[3];
    fast_mut!((table)[3]).bits = 3;
    fast_mut!((table)[7]).bits = 3;
    table_size = 8;
  } else {
    return 0;
  }
  while table_size != goal_size {
    for index in 0..table_size {
      fast_mut!((table)[(table_size + index) as usize]) = fast!((table)[index as usize]);
    }
    table_size <<= 1;
  }
  goal_size
}
