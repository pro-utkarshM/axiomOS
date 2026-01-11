//! ARM64 Physical Memory Allocator
//!
//! Two-stage physical memory allocator:
//! - Stage 1: Simple bump allocator for early boot (before heap)
//! - Stage 2: Bitmap allocator for full memory management (after heap)

use core::sync::atomic::{AtomicUsize, Ordering};

use spin::Mutex;

use super::dtb;
use super::mem::{PAGE_SIZE, PAGE_SHIFT};

/// Physical frame number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysFrame(usize);

impl PhysFrame {
    /// Create a frame from a physical address (must be page-aligned)
    pub const fn from_addr(addr: usize) -> Self {
        debug_assert!(addr & (PAGE_SIZE - 1) == 0, "Address not page-aligned");
        Self(addr >> PAGE_SHIFT)
    }

    /// Create a frame from a frame number
    pub const fn from_number(num: usize) -> Self {
        Self(num)
    }

    /// Get the physical address of this frame
    pub const fn addr(&self) -> usize {
        self.0 << PAGE_SHIFT
    }

    /// Get the frame number
    pub const fn number(&self) -> usize {
        self.0
    }
}

/// Early boot bump allocator (Stage 1)
///
/// Simple allocator that bumps through available memory regions.
/// Does not support deallocation - only used before heap is available.
struct BumpAllocator {
    /// Current region index in DTB memory regions
    current_region: usize,
    /// Next frame to allocate within current region
    next_frame_in_region: usize,
    /// Total frames allocated
    total_allocated: usize,
}

impl BumpAllocator {
    const fn new() -> Self {
        Self {
            current_region: 0,
            next_frame_in_region: 0,
            total_allocated: 0,
        }
    }

    /// Allocate a single physical frame
    fn allocate(&mut self) -> Option<PhysFrame> {
        let info = dtb::info();

        loop {
            if self.current_region >= info.memory_region_count {
                return None;
            }

            let region = info.memory_regions[self.current_region].as_ref()?;
            let region_frames = region.size / PAGE_SIZE;

            // Skip kernel region (first 16MB to be safe)
            let skip_frames = if region.base < 0x100_0000 {
                (0x100_0000 - region.base) / PAGE_SIZE
            } else {
                0
            };

            let available_start = skip_frames;
            let available_frames = region_frames.saturating_sub(skip_frames);

            if self.next_frame_in_region < available_frames {
                let frame_addr = region.base + (available_start + self.next_frame_in_region) * PAGE_SIZE;
                self.next_frame_in_region += 1;
                self.total_allocated += 1;
                return Some(PhysFrame::from_addr(frame_addr));
            }

            // Move to next region
            self.current_region += 1;
            self.next_frame_in_region = 0;
        }
    }

    /// Get total frames allocated so far
    fn allocated_count(&self) -> usize {
        self.total_allocated
    }
}

/// Physical memory allocator state
enum Allocator {
    /// Not initialized yet
    Uninitialized,
    /// Stage 1: Bump allocator (early boot)
    Stage1(BumpAllocator),
    /// Stage 2: Bitmap allocator (after heap init)
    Stage2(BitmapAllocator),
}

/// Global physical memory allocator
static ALLOCATOR: Mutex<Allocator> = Mutex::new(Allocator::Uninitialized);

/// Total usable physical memory in bytes
static TOTAL_MEMORY: AtomicUsize = AtomicUsize::new(0);

/// Initialize stage 1 (bump allocator)
///
/// Call this after DTB parsing, before heap initialization.
pub fn init_stage1() {
    let info = dtb::info();
    let total = info.total_memory;

    TOTAL_MEMORY.store(total, Ordering::SeqCst);

    let mut alloc = ALLOCATOR.lock();
    *alloc = Allocator::Stage1(BumpAllocator::new());

    log::info!(
        "Physical memory stage 1 initialized: {} MB available",
        total / (1024 * 1024)
    );
}

/// Initialize stage 2 (bitmap allocator)
///
/// Call this after heap is initialized.
pub fn init_stage2() {
    let mut alloc = ALLOCATOR.lock();

    let allocated_count = match &*alloc {
        Allocator::Stage1(bump) => bump.allocated_count(),
        _ => panic!("init_stage2 called without stage1"),
    };

    let bitmap = BitmapAllocator::new(allocated_count);
    *alloc = Allocator::Stage2(bitmap);

    log::info!(
        "Physical memory stage 2 initialized: {} frames pre-allocated",
        allocated_count
    );
}

/// Allocate a single physical frame
pub fn allocate_frame() -> Option<PhysFrame> {
    let mut alloc = ALLOCATOR.lock();
    match &mut *alloc {
        Allocator::Uninitialized => {
            log::warn!("Physical allocator not initialized");
            None
        }
        Allocator::Stage1(bump) => bump.allocate(),
        Allocator::Stage2(bitmap) => bitmap.allocate(),
    }
}

/// Deallocate a physical frame
///
/// Only works in stage 2. Panics if called in stage 1.
pub fn deallocate_frame(frame: PhysFrame) {
    let mut alloc = ALLOCATOR.lock();
    match &mut *alloc {
        Allocator::Uninitialized => panic!("Physical allocator not initialized"),
        Allocator::Stage1(_) => panic!("Cannot deallocate in stage 1"),
        Allocator::Stage2(bitmap) => bitmap.deallocate(frame),
    }
}

/// Get total usable physical memory
pub fn total_memory() -> usize {
    TOTAL_MEMORY.load(Ordering::SeqCst)
}

/// Check if allocator is initialized
pub fn is_initialized() -> bool {
    let alloc = ALLOCATOR.lock();
    !matches!(&*alloc, Allocator::Uninitialized)
}

/// Bitmap allocator for stage 2
///
/// Uses a bitmap to track free/allocated frames.
struct BitmapAllocator {
    /// Bitmap storage (allocated on heap)
    /// Each bit represents one frame: 0 = free, 1 = allocated
    bitmap: alloc::vec::Vec<u64>,
    /// First region base address
    base_addr: usize,
    /// Total number of frames tracked
    total_frames: usize,
    /// Number of free frames
    free_frames: usize,
    /// Hint for next free frame search
    next_hint: usize,
}

extern crate alloc;

impl BitmapAllocator {
    /// Create a new bitmap allocator
    ///
    /// `pre_allocated` is the number of frames already allocated by stage 1
    fn new(pre_allocated: usize) -> Self {
        let info = dtb::info();

        // Find total frames and base address
        let mut base_addr = usize::MAX;
        let mut total_frames = 0usize;

        for region in info.memory_regions() {
            if region.base < base_addr {
                base_addr = region.base;
            }
            total_frames += region.size / PAGE_SIZE;
        }

        // Skip first 16MB (kernel area)
        let skip_frames = if base_addr < 0x100_0000 {
            (0x100_0000 - base_addr) / PAGE_SIZE
        } else {
            0
        };

        base_addr = base_addr.max(0x100_0000);
        total_frames = total_frames.saturating_sub(skip_frames);

        // Allocate bitmap
        let bitmap_size = (total_frames + 63) / 64;
        let mut bitmap = alloc::vec![0u64; bitmap_size];

        // Mark pre-allocated frames as used
        for i in 0..pre_allocated {
            let word = i / 64;
            let bit = i % 64;
            if word < bitmap.len() {
                bitmap[word] |= 1 << bit;
            }
        }

        let free_frames = total_frames.saturating_sub(pre_allocated);

        log::info!(
            "Bitmap allocator: {} total frames, {} free, {} pre-allocated",
            total_frames,
            free_frames,
            pre_allocated
        );

        Self {
            bitmap,
            base_addr,
            total_frames,
            free_frames,
            next_hint: pre_allocated,
        }
    }

    /// Allocate a single frame
    fn allocate(&mut self) -> Option<PhysFrame> {
        if self.free_frames == 0 {
            return None;
        }

        // Start searching from hint
        let start_word = self.next_hint / 64;
        let total_words = self.bitmap.len();

        for offset in 0..total_words {
            let word_idx = (start_word + offset) % total_words;
            let word = self.bitmap[word_idx];

            // Find first zero bit
            if word != u64::MAX {
                let bit = (!word).trailing_zeros() as usize;
                let frame_idx = word_idx * 64 + bit;

                if frame_idx < self.total_frames {
                    self.bitmap[word_idx] |= 1 << bit;
                    self.free_frames -= 1;
                    self.next_hint = frame_idx + 1;

                    let addr = self.base_addr + frame_idx * PAGE_SIZE;
                    return Some(PhysFrame::from_addr(addr));
                }
            }
        }

        None
    }

    /// Deallocate a frame
    fn deallocate(&mut self, frame: PhysFrame) {
        let addr = frame.addr();
        if addr < self.base_addr {
            log::warn!("Attempted to deallocate frame below base address");
            return;
        }

        let frame_idx = (addr - self.base_addr) / PAGE_SIZE;
        if frame_idx >= self.total_frames {
            log::warn!("Attempted to deallocate frame beyond tracked range");
            return;
        }

        let word_idx = frame_idx / 64;
        let bit = frame_idx % 64;

        if self.bitmap[word_idx] & (1 << bit) == 0 {
            log::warn!("Double-free detected for frame at {:#x}", addr);
            return;
        }

        self.bitmap[word_idx] &= !(1 << bit);
        self.free_frames += 1;

        // Update hint if this frame is before current hint
        if frame_idx < self.next_hint {
            self.next_hint = frame_idx;
        }
    }
}
