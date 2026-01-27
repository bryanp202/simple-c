use core::slice;
use std::{
    alloc::{Allocator, Global},
    cell::{Cell, RefCell},
    cmp,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ptr::{self, NonNull},
};

const PAGE: usize = 4096;
const HUGE_PAGE: usize = 2 * 1024 * 1024;

struct ArenaChunk<'a, T, A: Allocator = Global> {
    storage: NonNull<[MaybeUninit<T>]>,
    entries: usize,
    alloc: &'a A,
}

unsafe impl<#[may_dangle] T, A: Allocator> Drop for ArenaChunk<'_, T, A> {
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw_in(self.storage.as_mut(), &self.alloc)) }
    }
}

impl<'a, T, A: Allocator> ArenaChunk<'a, T, A> {
    #[inline]
    unsafe fn new_in(capacity: usize, alloc: &'a A) -> ArenaChunk<'a, T, A> {
        ArenaChunk {
            storage: NonNull::from(Box::leak(Box::new_uninit_slice_in(capacity, alloc))),
            entries: 0,
            alloc,
        }
    }

    /// Caller must ensure that `len` elements of this chunk have been initialized
    #[inline]
    unsafe fn destroy(&mut self, len: usize) {
        if std::mem::needs_drop::<T>() {
            unsafe {
                let slice = self.storage.as_mut();
                for item in &mut slice[0..len] {
                    item.assume_init_drop();
                }
            }
        }
    }

    #[inline]
    fn start(&mut self) -> *mut T {
        self.storage.as_ptr().cast()
    }

    #[inline]
    fn end(&mut self) -> *mut T {
        unsafe {
            if size_of::<T>() == 0 {
                ptr::without_provenance_mut(!0)
            } else {
                self.start().add(self.storage.len())
            }
        }
    }
}

/// A simple bump arena
///
/// Safety:
/// - Not safe for multithreading
/// - Types with `alignment > 16` may lead to unnecessary allocations
pub struct Arena<'a, A: Allocator = Global> {
    ptr: Cell<*mut u8>,
    end: Cell<*mut u8>,
    chunks: RefCell<Vec<ArenaChunk<'a, u8, A>>>,
    alloc: &'a A,
}

impl Default for Arena<'static, Global> {
    fn default() -> Self {
        Self::new()
    }
}

impl Arena<'static, Global> {
    #[inline]
    pub fn new() -> Arena<'static, Global> {
        Self::new_in(&Global)
    }
}

impl<'a, A: Allocator> Arena<'a, A> {
    #[inline]
    pub fn new_in(alloc: &'a A) -> Self {
        Self {
            ptr: Cell::new(ptr::null_mut()),
            end: Cell::new(ptr::null_mut()),
            chunks: RefCell::default(),
            alloc,
        }
    }

    #[inline]
    fn grow(&self, additional: usize) {
        let mut chunks = self.chunks.borrow_mut();
        let new_cap = if let Some(last_chunk) = chunks.last_mut() {
            last_chunk.storage.len().min(HUGE_PAGE / 2) * 2
        } else {
            PAGE
        };
        let capacity = cmp::max(additional, new_cap);

        let mut chunk = unsafe { ArenaChunk::new_in(capacity, self.alloc) };
        self.ptr.set(chunk.start());
        self.end.set(chunk.end());
        chunks.push(chunk);
    }

    #[inline]
    fn can_allocate(&self, additional: usize) -> bool {
        let available_bytes = unsafe { self.end.get().offset_from_unsigned(self.ptr.get()) };
        available_bytes >= additional
    }
}

unsafe impl<A: Allocator> Allocator for Arena<'_, A> {
    fn allocate(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<NonNull<[u8]>, std::alloc::AllocError> {
        loop {
            let ptr = self.ptr.get();
            let offset = ptr.align_offset(layout.align());
            unsafe { self.ptr.set(ptr.byte_add(offset)) };
            if self.ptr >= self.end || !self.can_allocate(layout.size()) {
                self.grow(layout.size());
            } else {
                break;
            }
        }

        let ptr = self.ptr.get();
        unsafe {
            self.ptr.set(self.ptr.get().add(layout.size()));
            Ok(NonNull::from(slice::from_raw_parts_mut(ptr, layout.size())))
        }
    }

    /// nop
    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: std::alloc::Layout) {}
}

pub struct TypedArena<'a, T, A: Allocator = Global> {
    ptr: Cell<*mut T>,
    end: Cell<*mut T>,
    chunks: RefCell<Vec<ArenaChunk<'a, T, A>>>,
    alloc: &'a A,
    _own: PhantomData<T>,
}

unsafe impl<'a, #[may_dangle] T, A: Allocator> Drop for TypedArena<'a, T, A> {
    fn drop(&mut self) {
        unsafe {
            let mut chunks_borrow = self.chunks.borrow_mut();
            if let Some(mut last_chunk) = chunks_borrow.pop() {
                self.clear_last_chunk(&mut last_chunk);
                for chunk in chunks_borrow.iter_mut() {
                    chunk.destroy(chunk.entries);
                }
            }
        }
    }
}

impl<T> Default for TypedArena<'static, T, Global> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TypedArena<'static, T, Global> {
    #[inline]
    pub fn new() -> Self {
        Self::new_in(&Global)
    }
}

impl<'a, T, A: Allocator> TypedArena<'a, T, A> {
    #[inline]
    pub fn new_in(alloc: &'a A) -> Self {
        TypedArena {
            ptr: Cell::new(ptr::null_mut()),
            end: Cell::new(ptr::null_mut()),
            chunks: RefCell::default(),
            alloc,
            _own: PhantomData,
        }
    }

    #[inline]
    pub fn alloc(&self, item: T) -> &mut T {
        if self.ptr == self.end {
            self.grow(1);
        }

        unsafe {
            if size_of::<T>() == 0 {
                self.ptr.set(self.ptr.get().wrapping_byte_add(1));
                let ptr = ptr::NonNull::<T>::dangling().as_ptr();
                ptr::write(ptr, item);
                &mut *ptr
            } else {
                let ptr = self.ptr.get();
                self.ptr.set(self.ptr.get().add(1));
                ptr::write(ptr, item);
                &mut *ptr
            }
        }
    }

    #[inline]
    fn grow(&self, additional: usize) {
        let elem_size = cmp::max(1, size_of::<T>());
        let mut chunks = self.chunks.borrow_mut();
        let new_cap = if let Some(last_chunk) = chunks.last_mut() {
            if mem::needs_drop::<T>() {
                let used_bytes = self.ptr.get().addr() - last_chunk.start().addr();
                last_chunk.entries = used_bytes / size_of::<T>();
            }
            last_chunk.storage.len().min(HUGE_PAGE / elem_size / 2) * 2
        } else {
            PAGE / elem_size
        };
        let capacity = cmp::max(additional, new_cap);

        let mut chunk = unsafe { ArenaChunk::new_in(capacity, self.alloc) };
        self.ptr.set(chunk.start());
        self.end.set(chunk.end());
        chunks.push(chunk);
    }

    fn clear_last_chunk(&self, last_chunk: &mut ArenaChunk<'a, T, A>) {
        let start = last_chunk.start();
        let end = self.ptr.get();

        unsafe {
            let diff = if size_of::<T>() == 0 {
                end.offset_from_unsigned(start)
            } else {
                end.offset_from_unsigned(start) / size_of::<T>()
            };
            last_chunk.destroy(diff);
        }
        self.ptr.set(last_chunk.start());
    }
}

#[test]
fn type_alloc_test() {
    let arena = TypedArena::new();
    let c = arena.alloc('a');
    assert_eq!(*c, 'a');
}

#[test]
fn arena_alloc_test() {
    let arena = Arena::new();
    let mut nums = Vec::new();
    let mut floats = Vec::new();

    for num in 0..10_000 {
        nums.push(Box::new_in(num, &arena));
        floats.push(Box::new_in(num as f32, &arena));
    }

    for (expected, (num_actual, float_actual)) in nums.into_iter().zip(floats).enumerate() {
        assert_eq!(expected, *num_actual);
        assert_eq!(expected as f32, *float_actual);
    }
}

#[test]
fn large_align_alloc_test() {
    #[repr(align(4096))]
    struct LargeBool(bool);

    let arena = Arena::new();
    let mut large_bools = Vec::new();

    for i in 0..256usize {
        large_bools.push(Box::new_in(LargeBool(i.is_multiple_of(2)), &arena));
    }

    for (i, actual) in large_bools.into_iter().enumerate() {
        assert_eq!(i.is_multiple_of(2), actual.0);
    }
}
