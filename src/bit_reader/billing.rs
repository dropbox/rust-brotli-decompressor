use core::clone::Clone;
#[cfg(feature="billing")]
mod bill {
    pub use std::collections::HashMap;
    pub use std::collections::hash_map::Entry;
}
#[cfg(feature="billing")]
use std;

#[allow(dead_code)]
#[derive(Debug, Clone, Ord, Eq, PartialEq, PartialOrd, Hash)]
pub enum Categories {
    Uncompressed,
    MetablockHeader,
    BlockTypeMetadata,
    LiteralContextMode,
    DistanceContextMode,
    LiteralHuffmanTable,
    InsertCopyHuffmanTable,
    DistanceHuffmanTable,
    Literals,
    ComplexLiterals,
    CopyLength,
    CopyDistance,
    DictLength,
    DictIndex,
    Misc
}

#[cfg(not(feature="billing"))]
#[derive(Clone,Default)]
pub struct Billing;


#[cfg(feature="billing")]
#[derive(Clone)]
pub struct Billing {
    category: Categories,
    pending_bill:bill::HashMap<Categories,u64>,
    bill:bill::HashMap<Categories,u64>,
}


#[cfg(feature="billing")]
impl Default for Billing {
    fn default() ->Self {
        Billing{
            category:Categories::Misc,
            pending_bill:bill::HashMap::<Categories, u64>::new(),
            bill:bill::HashMap::<Categories, u64>::new(),
        }
    }
}
#[cfg(feature="billing")]
impl Billing {
    pub fn tally(&mut self, count:u64) {
        match self.category {
            Categories::Misc => {
            },
            _ => {},
        }
        let counter = self.pending_bill.entry(self.category.clone()).or_insert(0);
        *counter += count;
    }
    pub fn remap(&mut self, old:Categories, fixed:Categories) {
        let mut delta = 0u64;
        {
            match self.pending_bill.entry(old.clone()) {
                bill::Entry::Occupied(ref mut counter) => {
                    delta = *counter.get();
                    *counter.get_mut() = 0;
                },
                _ => {},
            }
        }
        if delta != 0 {
            let counter = self.pending_bill.entry(fixed.clone()).or_insert(0);
            *counter += delta;
        }
    }
    pub fn commit(&mut self) {
        for (k, v) in self.pending_bill.iter_mut() {
            let counter = self.bill.entry(k.clone()).or_insert(0);
            *counter += *v;
            *v = 0;
        }
    }
    pub fn push_attrib(&mut self, categories:Categories) {
        assert_eq!(self.category, Categories::Misc);
        self.category = categories;
    }
    pub fn restore_attrib(&mut self, categories:Categories) {
        match self.category {
          Categories::Misc => assert!(false),
          _ => {},
        }
        self.category = categories;
    }
    pub fn pop_attrib(&mut self) {
        self.category = Categories::Misc;
    }
    pub fn get_attrib(&self) -> Categories {
        self.category.clone()
    }
    pub fn print_stderr(&mut self) {
        self.print(&mut std::io::stderr());
    }
    pub fn print<W:std::io::Write> (&mut self, writer: &mut W) {
        self.commit();
        writeln!(writer, "BILL").unwrap();
        for (k, v) in self.bill.iter() {
            writeln!(writer, "{:>9} {:>13.3}  {:?}", *v, (*v as f64) / 8.0, *k).unwrap();
        }
    }
}
#[cfg(not(feature="billing"))]
impl Billing {
    #[inline(always)]
    pub fn tally(&mut self, _count:u64) {}
    #[inline(always)]
    pub fn remap(&mut self, _old:Categories, _fixed:Categories) {}
    #[inline(always)]
    pub fn commit(&mut self) {}
    #[inline(always)]
    pub fn push_attrib(&mut self, _categories:Categories){}
    #[inline(always)]
    pub fn restore_attrib(&mut self, _categories:Categories){}
    #[inline(always)]
    pub fn pop_attrib(&mut self){}
    #[inline(always)]
    pub fn get_attrib(&self) -> Categories {
        Categories::Misc
    }
    #[inline(always)]
    pub fn print_stderr(&mut self) {}
}

