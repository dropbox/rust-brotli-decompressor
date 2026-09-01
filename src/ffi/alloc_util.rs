use core;
#[cfg(feature="std")]
use std;
use ::alloc;
use super::interface::{c_void, CAllocator};
#[cfg(feature="std")]
use std::vec::Vec;
#[cfg(feature="std")]
pub use std::boxed::Box;

#[cfg(feature="std")]
pub struct MemoryBlock<Ty:Sized+Default>(Box<[Ty]>);
#[cfg(feature="std")]
impl<Ty:Sized+Default> Default for MemoryBlock<Ty> {
    fn default() -> Self {
        MemoryBlock(Vec::<Ty>::new().into_boxed_slice())
    }
}
#[cfg(feature="std")]
impl<Ty:Sized+Default> alloc::SliceWrapper<Ty> for MemoryBlock<Ty> {
    fn slice(&self) -> &[Ty] {
        &self.0[..]
    }
}
#[cfg(feature="std")]
impl<Ty:Sized+Default> alloc::SliceWrapperMut<Ty> for MemoryBlock<Ty> {
    fn slice_mut(&mut self) -> &mut [Ty] {
        &mut self.0[..]
    }
}
#[cfg(feature="std")]
impl<Ty:Sized+Default> core::ops::Index<usize> for MemoryBlock<Ty> {
    type Output = Ty;
    fn index(&self, index:usize) -> &Ty {
        &self.0[index]
    }
}
#[cfg(feature="std")]
impl<Ty:Sized+Default> core::ops::IndexMut<usize> for MemoryBlock<Ty> {

    fn index_mut(&mut self, index:usize) -> &mut Ty {
        &mut self.0[index]
    }
}
#[cfg(feature="std")]
impl<Ty:Sized+Default> Drop for MemoryBlock<Ty> {
    fn drop (&mut self) {
        if self.0.len() != 0 {
            print!("leaking memory block of length {} element size: {}\n", self.0.len(), core::mem::size_of::<Ty>());

            let to_forget = core::mem::replace(self, MemoryBlock::default());
            core::mem::forget(to_forget);// leak it -- it's the only safe way with custom allocators
        }
    }
}
pub struct SubclassableAllocator {
    alloc: CAllocator
    // have alternative ty here
}

impl SubclassableAllocator {
    pub unsafe fn new(sub_alloc:CAllocator) -> Self {
        SubclassableAllocator{
            alloc:sub_alloc,
        }
    }
}
#[cfg(feature="std")]
impl<Ty:Sized+Default+Clone> alloc::Allocator<Ty> for SubclassableAllocator {
    type AllocatedMemory = MemoryBlock<Ty>;
    fn alloc_cell(&mut self, size:usize) ->MemoryBlock<Ty>{
        if size == 0 {
            return MemoryBlock::<Ty>::default();
        }
        if let Some(alloc_fn) = self.alloc.alloc_func {
            let alloc_size = match size.checked_mul(core::mem::size_of::<Ty>()) {
                Some(alloc_size) => alloc_size,
                None => return MemoryBlock::<Ty>::default(),
            };
            let ptr = alloc_fn(self.alloc.opaque, alloc_size);
            if ptr.is_null() {
                return MemoryBlock::<Ty>::default();
            }
            let typed_ptr = unsafe {core::mem::transmute::<*mut c_void, *mut Ty>(ptr)};
            let slice_ref = unsafe {super::slice_from_raw_parts_or_nil_mut(typed_ptr, size)};
            for item in slice_ref.iter_mut() {
                unsafe{core::ptr::write(item, Ty::default())};
            }
            return MemoryBlock(unsafe{Box::from_raw(slice_ref)})
        }
        MemoryBlock(vec![Ty::default();size].into_boxed_slice())
    }
    fn free_cell(&mut self, mut bv:MemoryBlock<Ty>) {
        if (*bv.0).len() != 0 {
            if let Some(_) = self.alloc.alloc_func {
                let slice_ptr = (*bv.0).as_mut_ptr();
                let _box_ptr = Box::into_raw(core::mem::replace(&mut bv.0, Vec::<Ty>::new().into_boxed_slice()));
                if let Some(free_fn) = self.alloc.free_func {
                    unsafe {free_fn(self.alloc.opaque, core::mem::transmute::<*mut Ty, *mut c_void>(slice_ptr))};
                }
            } else {
                let _to_free = core::mem::replace(&mut bv.0, Vec::<Ty>::new().into_boxed_slice());
            }
        }
    }
}











#[cfg(not(feature="std"))]
pub struct MemoryBlock<Ty:Sized+Default>(*mut[Ty]);
#[cfg(not(feature="std"))]
impl<Ty:Sized+Default> Default for MemoryBlock<Ty> {
    fn default() -> Self {
        // Even an empty slice reference needs a non-null, Ty-aligned pointer.
        MemoryBlock(core::ptr::slice_from_raw_parts_mut(
            core::ptr::NonNull::<Ty>::dangling().as_ptr(), 0))
    }
}
#[cfg(not(feature="std"))]
impl<Ty:Sized+Default> alloc::SliceWrapper<Ty> for MemoryBlock<Ty> {
    fn slice(&self) -> &[Ty] {
        // The pointer is either an aligned empty slice or an initialized
        // allocation from alloc_cell; the borrow cannot outlive this block.
        unsafe { &*self.0 }
    }
}
#[cfg(not(feature="std"))]
impl<Ty:Sized+Default> alloc::SliceWrapperMut<Ty> for MemoryBlock<Ty> {
    fn slice_mut(&mut self) -> &mut [Ty] {
        // As above, with exclusive access guaranteed by the mutable borrow.
        unsafe { &mut *self.0 }
    }
}

#[cfg(not(feature="std"))]
#[cfg(feature="no-stdlib-ffi-binding")]
#[panic_handler]
extern fn panic_impl(_: &::core::panic::PanicInfo) -> ! {
    loop {}
}
#[cfg(not(feature="std"))]
#[cfg(feature="no-stdlib-ffi-binding")]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
}

#[cfg(not(feature="std"))]
impl<Ty:Sized+Default> core::ops::Index<usize> for MemoryBlock<Ty> {
    type Output = Ty;
    fn index(&self, index:usize) -> &Ty {
        &alloc::SliceWrapper::slice(self)[index]
    }
}
#[cfg(not(feature="std"))]
impl<Ty:Sized+Default> core::ops::IndexMut<usize> for MemoryBlock<Ty> {

    fn index_mut(&mut self, index:usize) -> &mut Ty {
        &mut alloc::SliceWrapperMut::slice_mut(self)[index]
    }
}

#[cfg(not(feature="std"))]
impl<Ty:Sized+Default+Clone> alloc::Allocator<Ty> for SubclassableAllocator {
    type AllocatedMemory = MemoryBlock<Ty>;
    fn alloc_cell(&mut self, size:usize) ->MemoryBlock<Ty>{
        if size == 0 {
            return MemoryBlock::<Ty>::default();
        }
        if let Some(alloc_fn) = self.alloc.alloc_func {
            let alloc_size = match size.checked_mul(core::mem::size_of::<Ty>()) {
                Some(alloc_size) => alloc_size,
                None => return MemoryBlock::<Ty>::default(),
            };
            let ptr = alloc_fn(self.alloc.opaque, alloc_size);
            if ptr.is_null() {
                return MemoryBlock::<Ty>::default();
            }
            let typed_ptr = unsafe {core::mem::transmute::<*mut c_void, *mut Ty>(ptr)};
            let slice_ref = unsafe {super::slice_from_raw_parts_or_nil_mut(typed_ptr, size)};
            for item in slice_ref.iter_mut() {
                unsafe{core::ptr::write(item, Ty::default())};
            }
            return MemoryBlock(slice_ref)
        } else {
            panic!("Must provide allocators in no-stdlib code");
        }
    }
    fn free_cell(&mut self, mut bv:MemoryBlock<Ty>) {
        use alloc::SliceWrapper;
        use alloc::SliceWrapperMut;
        if bv.slice().len() != 0 {
            if let Some(_) = self.alloc.alloc_func {
                if let Some(free_fn) = self.alloc.free_func {
                    unsafe {free_fn(self.alloc.opaque, core::mem::transmute::<*mut Ty, *mut c_void>(&mut bv.slice_mut()[0]))};
                }
                let _ = core::mem::replace(&mut bv,
                                           MemoryBlock::<Ty>::default());
            } else {
                panic!("Must provide allocators in no-stdlib code");
            }
        }
    }
}


#[cfg(not(feature="std"))]
pub fn free_stdlib<T>(_data: *mut T, _size: usize) {
    panic!("Must supply allocators if calling divans when compiled with features=no-stdlib");
}
#[cfg(not(feature="std"))]
pub fn alloc_stdlib<T:Sized+Default+Copy+Clone>(_size: usize) -> *mut T {
    panic!("Must supply allocators if calling divans when compiled with features=no-stdlib");
}

#[cfg(feature="std")]
pub unsafe fn free_stdlib<T>(ptr: *mut T, size: usize) {
    if ptr.is_null() {
        return;
    }
    let slice_ref = super::slice_from_raw_parts_or_nil_mut(ptr, size);
    let _ = Box::from_raw(slice_ref); // free on drop
}
#[cfg(feature="std")]
pub fn alloc_stdlib<T:Sized+Default+Copy+Clone>(size: usize) -> *mut T {
    std::panic::catch_unwind(|| {
        let mut newly_allocated = vec![T::default();size].into_boxed_slice();
        let slice_ptr = newly_allocated.as_mut_ptr();
        let _box_ptr = Box::into_raw(newly_allocated);
        slice_ptr
    }).unwrap_or(core::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::alloc::{Allocator, SliceWrapper, SliceWrapperMut};

    #[repr(align(64))]
    #[derive(Clone, Default)]
    struct OverAligned {
        _byte: u8,
    }

    fn assert_empty_block<Ty: Sized + Default>(mut block: MemoryBlock<Ty>) {
        #[cfg(not(feature="std"))]
        {
            // Inspect the raw pointer first, so a regression fails without
            // creating an invalid reference just to check the slice length.
            let data = block.0 as *mut Ty;
            assert!(!data.is_null());
            assert_eq!(data as usize % core::mem::align_of::<Ty>(), 0);
        }
        assert!(block.slice().is_empty());
        assert_eq!(block.slice().as_ptr() as usize % core::mem::align_of::<Ty>(), 0);
        let slice = block.slice_mut();
        assert!(slice.is_empty());
        assert_eq!(slice.as_mut_ptr() as usize % core::mem::align_of::<Ty>(), 0);
    }

    #[test]
    fn default_empty_blocks_are_aligned_for_their_element_type() {
        assert_empty_block(MemoryBlock::<u8>::default());
        assert_empty_block(MemoryBlock::<u32>::default());
        assert_empty_block(MemoryBlock::<super::super::HuffmanCode>::default());
        assert_empty_block(MemoryBlock::<OverAligned>::default());
    }

    #[cfg(not(feature="std"))]
    #[test]
    fn nonempty_block_supports_slices_and_indexing() {
        let mut data = [1u32, 2, 3];
        {
            let mut block = MemoryBlock(&mut data[..] as *mut [u32]);
            assert_eq!(block.slice(), &[1, 2, 3]);
            assert_eq!(block[1], 2);
            block[1] = 4;
            block.slice_mut()[2] = 5;
            assert_eq!(block.slice(), &[1, 4, 5]);
        }
        assert_eq!(data, [1, 4, 5]);
    }

    extern "C" fn failing_alloc(_data: *mut c_void, _size: usize) -> *mut c_void {
        core::ptr::null_mut()
    }

    #[test]
    fn failed_custom_allocation_returns_empty_block() {
        let c_allocator = CAllocator {
            alloc_func: Some(failing_alloc),
            free_func: None,
            opaque: core::ptr::null_mut(),
        };
        let mut allocator = unsafe { SubclassableAllocator::new(c_allocator) };
        let block =
            <SubclassableAllocator as Allocator<u8>>::alloc_cell(&mut allocator, 1);

        assert_eq!(block.slice().len(), 0);
    }

    #[test]
    fn zero_and_failed_allocations_return_aligned_empty_blocks() {
        let mut allocator = unsafe {
            SubclassableAllocator::new(CAllocator {
                alloc_func: Some(failing_alloc),
                free_func: None,
                opaque: core::ptr::null_mut(),
            })
        };
        for &size in &[0usize, 1, usize::MAX] {
            let block = <SubclassableAllocator as Allocator<OverAligned>>::alloc_cell(
                &mut allocator, size);
            assert_empty_block(block);
        }
    }
}
