use core::{cell::UnsafeCell, marker::PhantomData, ptr::NonNull, sync::atomic::AtomicBool};
use std::sync::{RwLock, RwLockWriteGuard};

use bevy_ptr::ThinSlicePtr;

use crate::{component::Tick, storage::thin_array_ptr::ThinArrayPtr};

// Dense ECS tick storage.
pub struct Ticks {
    // A fenwick tree storing min/max ranges of ticks. Each leaf in the
    // tree corresponds to a configurable "block size" of actual ticks. With B = 16,
    // the overall storage cost is ~12%.
    summary: ThinArrayPtr<RwLock<TickSummary>>,
    ticks: ThinArrayPtr<UnsafeCell<Tick>>,
}

pub struct TickSummary {
    max: Tick,
}

pub struct TickSummaryCursor<'a> {
    ptr: NonNull<RwLock<TickSummary>>,
    index: u32,
    len: u32,
    _data: PhantomData<&'a [TickSummary]>,
}

impl<'a> TickSummaryCursor<'a> {
    pub fn get(&self) -> &RwLock<TickSummary> {
        // SAFETY: `ptr` is kept valid at all times during this struct's lifetime
        unsafe { self.ptr.as_ref() }
    }

    pub fn to_update_parent(&mut self) -> bool {
        let offset = ops::lsb(self.index as i32);
        let new_index = self.index as i32 + offset;
        if (new_index >= self.len) {
            return false;
        }
        unsafe {
            self.ptr.offset(offset as isize);
        }

        self.index = new_index;
        true
    }
}

pub struct TickSummaryGuard<'a> {
    leaf: TickSummaryCursor<'a>,
    summary: TickSummary,
    changed: bool,
}

impl<'a> Drop for TickSummaryGuard<'a> {
    fn drop(&mut self) {
        if !self.changed {
            return;
        }

        // propagate summary changes upwards
    }
}

pub struct TicksSlice<'a> {
    summary: TickSummaryGuard<'a>,
    ticks: &'a mut [Tick],
}

mod ops {
    pub fn lsb(n: i32) -> i32 {
        n & (-n)
    }
}
