use core::cell::UnsafeCell;

use bevy_ptr::ThinSlicePtr;

use crate::{component::Tick, storage::thin_array_ptr::ThinArrayPtr};

// Dense ECS tick storage.
pub struct Ticks {
    // A fenwick tree storing min/max ranges of ticks. Each leaf in the
    // tree corresponds to a configurable "block size" of actual ticks. With B = 16,
    // the overall storage cost is ~12%.
    summary: ThinArrayPtr<UnsafeCell<TickSummary>>,
    ticks: ThinArrayPtr<UnsafeCell<Tick>>,
}

struct TickSummary {
    min: Tick,
    max: Tick,
}

impl Ticks {
    unsafe fn as_block(&self, height: u8) -> Block {
        todo!()
    }
}
