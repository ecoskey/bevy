use core::{cell::UnsafeCell, marker::PhantomData, ptr::NonNull};
use std::sync::RwLock;

use crate::{component::Tick, storage::thin_array_ptr::ThinArrayPtr};

// Dense ECS tick storage.
pub struct Ticks {
    // A fenwick tree storing min/max ranges of ticks. Each leaf in the
    // tree corresponds to a configurable "block size" of actual ticks. With B = 16,
    // the overall storage cost is ~12%.
    summary: ThinArrayPtr<RwLock<TickSummary>>,
    ticks: ThinArrayPtr<UnsafeCell<Tick>>,
}

impl Ticks {}

pub struct TickSummary {
    max: Tick,
}

#[derive(Copy, Clone)]
pub struct TickSummaryCursor<'a> {
    ptr: NonNull<RwLock<TickSummary>>,
    index: u32,
    len: u32,
    _data: PhantomData<&'a [RwLock<TickSummary>]>,
}

// SAFETY: TickSummaryCursor borrows Ticks.summary by reference, so there is no unsafe aliasing
unsafe impl<'a> Send for TickSummaryCursor<'a> {}
// SAFETY: TickSummaryCursor borrows Ticks.summary by reference, so there is no unsafe aliasing
unsafe impl<'a> Sync for TickSummaryCursor<'a> {}

impl<'a> TickSummaryCursor<'a> {
    pub fn get(&self) -> &RwLock<TickSummary> {
        // SAFETY: `ptr` is kept valid at all times during this struct's lifetime
        unsafe { self.ptr.as_ref() }
    }

    fn get_at_offset_right(self, offset: u32) -> Option<Self> {
        let mut new_cursor = self;
        let in_bounds = new_cursor.len - new_cursor.index < offset;
        in_bounds.then(|| {
            // SAFETY: new_index is in bounds
            unsafe { new_cursor.ptr.offset(offset as isize) };
            new_cursor.index += offset;
            new_cursor
        })
    }

    fn get_at_offset_left(self, offset: u32) -> Option<Self> {
        let mut new_cursor = self;
        let in_bounds = new_cursor.index >= offset;
        in_bounds.then(|| {
            // SAFETY: new_index is in bounds
            unsafe { new_cursor.ptr.offset(-(offset as isize)) };
            new_cursor.index -= offset;
            new_cursor
        })
    }

    pub fn update(&mut self, other: &TickSummary, this_run: Tick) {
        let mut summary = self.get().write().unwrap(); //TODO: not unwrap
        if other.max.is_newer_than(summary.max, this_run) {
            summary.max = other.max;
        }
    }

    pub fn get_update_parent(self) -> Option<Self> {
        let offset = ops::lsb(self.index);
        self.get_at_offset_right(offset)
    }

    pub fn get_search_children(self) -> Option<(Self, Self)> {
        let offset = ops::lsb(self.index) / 2;
        let left = self.get_at_offset_left(offset);
        let right = self.get_at_offset_right(offset);
        left.zip(right)
    }
}

pub struct TicksSliceMut<'a> {
    summary: TickSummaryCursor<'a>,
    scratch: TickSummary,
    dirty: bool,
    ticks: &'a mut [Tick],
}

impl<'a> TicksSliceMut<'a> {
    pub fn drop(mut self, this_run: Tick) {
        if !self.dirty {
            return;
        }

        loop {
            self.summary.update(&self.scratch, this_run);
            if let Some(parent) = self.summary.get_update_parent() {
                self.summary = parent;
            }
        }
    }
}

impl<'a> Drop for TicksSliceMut<'a> {
    fn drop(&mut self) {
        panic!("TicksSliceMut dropped without explicit call to `drop`!");
    }
}

mod ops {
    pub fn lsb(n: u32) -> u32 {
        n & (!n).wrapping_add(1)
    }
}
