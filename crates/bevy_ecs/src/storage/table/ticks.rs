use core::{cell::UnsafeCell, sync::atomic::AtomicU32};

use bevy_ptr::ThinSlicePtr;

use crate::{component::Tick, storage::thin_array_ptr::ThinArrayPtr};

// Dense ECS tick storage.
pub struct Ticks {
    summary: ThinArrayPtr<UnsafeCell<TickSummary>>,
    ticks: ThinArrayPtr<UnsafeCell<Tick>>,
}

struct TickSummary {
    min: Tick,
    max: Tick,
    mask: u32,
}

impl Ticks {
    const B: u8 = 16;

    unsafe fn block(&self, height: u8) -> Block<B> {
        todo!()
    }
}

#[derive(Clone)]
struct Block<'a> {
    slice: ThinSlicePtr<'a, UnsafeCell<TickSummary>>,
    height: u8,
}

struct BlockSize {
    length: usize,
    height: u8,
}

impl<'a> Block<'a> {
    // Create a new `TicksBlock` from a thin slice
    //
    // # Safety:
    // - `ptr` must contain a valid packed B-ary tree in van Emde Boas layout.
    // - `height` must be equal to the height of the tree in `ptr`
    unsafe fn new(ptr: ThinSlicePtr<'a, UnsafeCell<Tick>>, height: u8) -> Self {
        Self { slice: ptr, height }
    }

    pub fn height(&self) -> u8 {
        self.height
    }

    pub fn len(&self) -> usize {
        ops::geometric_series(Ticks::B as usize, self.height as u32)
    }

    pub fn root(&self) -> &UnsafeCell<Tick> {
        unsafe { self.slice.get(0) }
    }

    pub unsafe fn outer(&self, info: BlockInfo) -> Self {
        match info {
            BlockInfo::Upper => {
                todo!()
            }
            BlockInfo::Lower(block_index) => {
                todo!()
            }
        }
    }

    //TODO: docs/safety comments

    pub fn upper(&self) -> Self {
        //TODO: this doesn't work for power of two block sizes, weirdly enough.
        let upper_height = self.height - ops::prev_power_of_two(self.height);
        let upper_len = ops::geometric_series(Ticks::B as usize, upper_height as u32);

        let upper_slice = unsafe { self.slice.offset(0, upper_len) };

        // SAFETY:
        unsafe { Block::new(upper_slice, upper_height) }
    }

    pub unsafe fn lower(&self, index: u8) -> Self {
        debug_assert!(
            index < Ticks::B,
            "called Block::lower() with index {} >= {}",
            index,
            Ticks::B
        );
        let lower_height = ops::prev_power_of_two(self.height);
        let lower_len = ops::geometric_series(Ticks::B as usize, lower_height as u32);
        let lower_slice = unsafe { self.slice.offset(lower_len * index as usize, lower_len) };

        unsafe { Block::new(lower_slice, lower_height) }
    }
}

#[derive(Copy, Clone)]
enum BlockInfo {
    Upper,
    Lower(u8),
}

#[derive(Clone)]
pub struct Cursor<'a> {
    block: Block<'a>,
    block_info: BlockInfo,
    global_height: u8,
}

impl<'a> Cursor<'a> {
    pub fn get(&self) -> &UnsafeCell<Tick> {
        self.block.root()
    }

    pub fn to_child(&mut self, index: u8) -> bool {
        assert!(
            index < Ticks::B,
            "called Cursor::to_child() with index {} >= {}",
            index,
            Ticks::B
        );
        todo!()
    }

    pub fn to_next_sibling(&mut self) -> bool {
        match self.block_info {
            BlockInfo::Upper => todo!(), //out, proceed as if lower block, down
            BlockInfo::Lower(i) if i == Ticks::B - 1 => todo!(), // out, next lower block of outside
            BlockInfo::Lower(i) => todo!(), //next
        }
    }

    pub fn to_prev_sibling(&mut self) -> bool {
        todo!()
    }

    pub fn to_parent(&mut self) -> bool {
        todo!()
    }
}

mod ops {
    /// Returns the greatest power of two less than or equal to `self`, or 0 otherwise.
    pub const fn prev_power_of_two(n: u8) -> u8 {
        // n = 0 gives highest_bit_set_idx = 0.
        let highest_bit_set_idx = 7 - (n | 1).leading_zeros();
        // Binary AND of highest bit with n is a no-op, except zero gets wiped.
        (1 << highest_bit_set_idx) & n
    }

    // geometric series: Σ i=0..n (B^i)
    #[inline]
    pub const fn geometric_series(b: usize, n: u32) -> usize {
        (b.pow(n + 1) - 1) / (b - 1)
    }
}
