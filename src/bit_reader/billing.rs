use core::clone::Clone;
#[cfg(feature="billing")]
mod bill {
    pub use std::collections::HashMap;
}
#[cfg(feature="billing")]
use std;

#[allow(dead_code)]
#[derive(Debug, Clone, Ord, Eq, PartialEq, PartialOrd, Hash)]
pub enum Categories {
    LiteralHuffmanTable,
    DistanceHuffmanTable,
    CodeHuffmanTable,
    Literals,
    CopyLength,
    CopyDistance,
    Misc
}

#[cfg(not(feature="billing"))]
#[derive(Clone,Default)]
pub struct Billing();


#[cfg(feature="billing")]
#[derive(Clone)]
pub struct Billing {
    categories: std::vec::Vec<Categories>,
    bill:bill::HashMap<Categories,u64>,
}


#[cfg(feature="billing")]
impl Default for Billing {
    fn default() ->Self {
        Billing{
            categories:vec![Categories::Misc],
            bill:bill::HashMap::<Categories, u64>::new(),
        }
    }
}
#[cfg(feature="billing")]
impl Billing {
    pub fn tally(&mut self, count:u64) {
        let counter = self.bill.entry(self.categories[self.categories.len() - 1].clone()).or_insert(0);
        *counter += count;
    }
    pub fn push_attrib(&mut self, categories:Categories) {
        self.categories.push(categories);
    }
    pub fn pop_attrib(&mut self) {
        assert!(self.categories.len() > 1);
        self.categories.pop();
    }
    pub fn print_stderr(&self) {
        self.print(&mut std::io::stderr());
    }
    pub fn print<W:std::io::Write> (&self, writer: &mut W) {
        writeln!(writer, "BILL").unwrap();
        for (k, v) in self.bill.iter() {
            writeln!(writer, "{:?}: {} {}", *k, *v, (*v as f64) / 8.0).unwrap();
        }
    }
}
#[cfg(not(feature="billing"))]
impl Billing {
    pub fn tally(&mut self, _count:u64) {}
    pub fn push_attrib(&mut self, _categories:Categories){}
    pub fn pop_attrib(&mut self){}
    pub fn print_stderr(&self) {}
}

