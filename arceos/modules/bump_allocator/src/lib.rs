#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    used_bytes: usize,
    used_pages: usize,
    allocations: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            used_bytes: 0,
            used_pages: 0,
            allocations: 0,
        }
    }

    /// Aligns `value` up to the given power-of-two alignment.
    fn align_up(value: usize, align: usize) -> usize {
        if align <= 1 {
            value
        } else {
            (value + align - 1) & !(align - 1)
        }
    }

    /// Aligns `value` down to the given power-of-two alignment.
    fn align_down(value: usize, align: usize) -> usize {
        if align <= 1 {
            value
        } else {
            value & !(align - 1)
        }
    }
}

impl<const SIZE: usize> Default for EarlyAllocator<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
        self.used_bytes = 0;
        self.used_pages = 0;
        self.allocations = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        // The bump allocator only manages one contiguous region.  Extend the
        // region if the new memory is adjacent to it; otherwise report an
        // error instead of silently ignoring memory.
        if self.end == 0 {
            self.init(start, size);
            return Ok(());
        }
        if start == self.end {
            self.end += size;
            self.p_pos += size;
            Ok(())
        } else {
            Err(AllocError::MemoryOverlap)
        }
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        if layout.size() == 0 {
            let pos = Self::align_up(self.b_pos, layout.align());
            return NonNull::new(pos as *mut u8).ok_or(AllocError::NoMemory);
        }

        let pos = Self::align_up(self.b_pos, layout.align());
        let end = pos.checked_add(layout.size()).ok_or(AllocError::NoMemory)?;
        if end > self.p_pos {
            return Err(AllocError::NoMemory);
        }

        self.b_pos = end;
        self.used_bytes += layout.size();
        self.allocations += 1;
        Ok(NonNull::new(pos as *mut u8).unwrap())
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        self.used_bytes = self.used_bytes.saturating_sub(layout.size());
        self.allocations = self.allocations.saturating_sub(1);
        // A bump allocator cannot reclaim an individual allocation.  When the
        // last live allocation is freed, the whole byte area can be reused.
        if self.allocations == 0 {
            self.b_pos = self.start;
            self.used_bytes = 0;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    fn available_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos)
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        if num_pages == 0 {
            return Err(AllocError::InvalidParam);
        }
        let size = num_pages.checked_mul(SIZE).ok_or(AllocError::NoMemory)?;
        let pos = self.p_pos.checked_sub(size).ok_or(AllocError::NoMemory)?;
        let pos = Self::align_down(pos, align_pow2);
        if pos < self.b_pos {
            return Err(AllocError::NoMemory);
        }
        self.p_pos = pos;
        self.used_pages += num_pages;
        Ok(pos)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // Pages allocated from the back of the region are never reclaimed by
        // a bump allocator.
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start) / SIZE
    }

    fn used_pages(&self) -> usize {
        self.used_pages
    }

    fn available_pages(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos) / SIZE
    }
}
