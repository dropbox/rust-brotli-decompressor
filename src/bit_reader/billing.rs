use core::clone::Clone;
#[cfg(features="billing")]
mod bill {
    use std::collections::HashMap;
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Categories {
    LiteralHuffmanTable,
    DistanceHuffmanTable,
    CodeHuffmanTable,
    Literals,
    CopyLength,
    CopyDistance,
    Misc
}

#[cfg(features="billing")]
#[derive(Clone)]
pub struct Billing {
    category: Categories,
    bill:HashMap<Categories,uint64>,
}

#[cfg(not(features="billing"))]
#[derive(Default,Clone)]
pub struct Billing();

#[cfg(features="billing")]
impl Default for Billing {
    fn default() ->Self {
        Billing{
            categories:Categories::Misc,
            bill:HashMap::<Categories, uint64>::default(),
        }
    }
}
#[cfg(features="billing")]
impl Billing {
    fn tally(&mut self, count:u64) {
        self.bill.insert(category, count)
    }
}

#[cfg(features="billing")]
macro_rules! bill{
    ($billing: expr, $count: expr) => {
        billing.tally(count as u64)
    }
}

#[cfg(not(features="billing"))]
macro_rules! bill{
    ($billing: expr, $count: expr) => {
    }
}
