use core::cell::UnsafeCell;

use bevy_ptr::ThinSlicePtr;

use crate::{component::Tick, storage::thin_array_ptr::ThinArrayPtr};

use super::TicksCursor;

// Dense ECS tick storage.
pub struct Ticks {
    // A fenwick tree storing min/max ranges of ticks. Each leaf in the
    // tree corresponds to a configurable "block size" of actual ticks. With B = 16,
    // the overall storage cost is ~12%.
    summary: ThinArrayPtr<UnsafeCell<TickSummary>>,
    ticks: ThinArrayPtr<UnsafeCell<Tick>>,
}

pub struct TickSummary {
    left_min: Tick,
    right_min: Tick,
    max: Tick,
}

impl TickSummary {}

impl Ticks {
    unsafe fn summary_cursor(&self, count: u32, index: u32) {
        TicksCursor {
            block: ThinSlicePtr::,
            block_info: todo!(),
            global_height: todo!(),
        }
    }
}

pub struct TickCursor<'a> {
    slice: ThinSlicePtr<'a, UnsafeCell<TickSummary>>,
    count: u32,
    index: u32,
}

impl<'a> Drop for TicksMut<'a> {
    fn drop(&mut self) {
        let mut new_range = TickSummary::NONE;
        for tick in self.block {
            new_range.update(*tick);
        }
    }
}
