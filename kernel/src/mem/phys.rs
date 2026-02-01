use alloc::vec::Vec;
use core::iter::from_fn;
use core::mem::swap;

use conquer_once::spin::OnceCell;
use kernel_physical_memory::{PhysicalFrameAllocator, PhysicalMemoryManager, FrameState};
#[cfg(target_arch = "x86_64")]
use limine::memory_map::{Entry, EntryType};
use log::{info, warn, error};
use spin::Mutex;

use crate::arch::types::{PageSize, PhysAddr, PhysFrame, PhysFrameRange, Size4KiB};
use crate::mem::heap::Heap;

static PHYS_ALLOC: OnceCell<Mutex<MultiStageAllocator>> = OnceCell::uninit();

struct ReservedRegions {
    regions: [MemoryRegion; 16],
    count: usize,
}

impl ReservedRegions {
    const fn new() -> Self {
        Self {
            regions: [MemoryRegion { base: 0, length: 0 }; 16],
            count: 0,
        }
    }

    fn push(&mut self, region: MemoryRegion) {
        if self.count < 16 {
            self.regions[self.count] = region;
            self.count += 1;
        } else {
            warn!("Too many reserved regions, ignoring some");
        }
    }

    fn is_reserved(&self, addr: u64) -> bool {
        for i in 0..self.count {
            let r = &self.regions[i];
            if addr >= r.base && addr < r.base + r.length {
                return true;
            }
        }
        false
    }
}

static RESERVED_REGIONS: Mutex<ReservedRegions> = Mutex::new(ReservedRegions::new());

fn allocator() -> &'static Mutex<MultiStageAllocator> {
    PHYS_ALLOC
        .get()
        .expect("physical allocator not initialized")
}

/// Zero-sized facade to the global physical memory allocator.
#[derive(Copy, Clone)]
pub struct PhysicalMemory;

#[allow(dead_code)]
impl PhysicalMemory {
    pub fn is_initialized() -> bool {
        PHYS_ALLOC.is_initialized()
    }

    pub fn allocate_frames_non_contiguous<S: PageSize>() -> impl Iterator<Item = PhysFrame<S>>
    where
        PhysicalMemoryManager: PhysicalFrameAllocator<S>,
    {
        from_fn(Self::allocate_frame)
    }

    #[must_use]
    pub fn allocate_frame<S: PageSize>() -> Option<PhysFrame<S>>
    where
        PhysicalMemoryManager: PhysicalFrameAllocator<S>,
    {
        allocator().lock().allocate_frame()
    }

    #[must_use]
    pub fn allocate_frames<S: PageSize>(n: usize) -> Option<PhysFrameRange<S>>
    where
        PhysicalMemoryManager: PhysicalFrameAllocator<S>,
    {
        allocator().lock().allocate_frames(n)
    }

    pub fn deallocate_frame<S: PageSize>(frame: PhysFrame<S>)
    where
        PhysicalMemoryManager: PhysicalFrameAllocator<S>,
    {
        allocator().lock().deallocate_frame(frame);
    }

    pub fn deallocate_frames<S: PageSize>(range: PhysFrameRange<S>)
    where
        PhysicalMemoryManager: PhysicalFrameAllocator<S>,
    {
        allocator().lock().deallocate_frames(range);
    }
}

/// Generic memory region for physical memory initialization
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base: u64,
    pub length: u64,
}

pub fn register_reserved_region(base: u64, length: u64) {
    RESERVED_REGIONS.lock().push(MemoryRegion { base, length });
}

#[cfg(target_arch = "x86_64")]
unsafe impl x86_64::structures::paging::FrameAllocator<Size4KiB> for PhysicalMemory {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        Self::allocate_frame()
    }
}

#[cfg(target_arch = "x86_64")]
static mut BOOT_REGIONS: [MemoryRegion; 128] = [MemoryRegion { base: 0, length: 0 }; 128];

#[cfg(target_arch = "x86_64")]
pub fn init_stage1(entries: &[&Entry]) -> usize {
    let usable_physical_memory = entries
        .iter()
        .filter(|e| e.entry_type == EntryType::USABLE)
        .map(|e| e.length)
        .sum::<u64>();
    info!("usable RAM: ~{} MiB", usable_physical_memory / 1024 / 1024);

    let mut count = 0;
    for entry in entries {
        if count < 128 {
            unsafe {
                BOOT_REGIONS[count] = MemoryRegion {
                    base: entry.base,
                    length: entry.length,
                };
            }
            count += 1;
        }
    }

    // SAFETY: We are in early boot, single-threaded.
    let regions_static: &'static [MemoryRegion] = unsafe { &BOOT_REGIONS[..count] };

    let stage1 = MultiStageAllocator::Stage1(PhysicalBumpAllocator::new(regions_static));
    PHYS_ALLOC.init_once(|| Mutex::new(stage1));

    usable_physical_memory as usize
}

pub fn init_stage1_aarch64(regions: &'static [MemoryRegion]) -> usize {
    let usable_physical_memory = regions.iter().map(|r| r.length).sum::<u64>();
    info!("usable RAM: ~{} MiB", usable_physical_memory / 1024 / 1024);

    let stage1 = MultiStageAllocator::Stage1(PhysicalBumpAllocator::new(regions));
    PHYS_ALLOC.init_once(|| Mutex::new(stage1));

    usable_physical_memory as usize
}

pub fn init_stage2() {
    let mut guard = allocator().lock();

    let MultiStageAllocator::Stage1(stage1) = &*guard else {
        unreachable!()
    };

    assert!(Heap::is_initialized());

    let regions = stage1.regions;
    let stage_one_next_free = stage1.next_frame;
    info!("Transitioning to stage 2. Stage 1 allocated {} frames", stage_one_next_free);

    let mut memory_regions = Vec::with_capacity(regions.len());

    for entry in regions {
        let num_frames = (entry.length / Size4KiB::SIZE) as usize;
        info!("Adding memory region: {:#x} - {:#x} ({} frames)", entry.base, entry.base + entry.length, num_frames);
        let region = kernel_physical_memory::MemoryRegion::new(
            entry.base,
            num_frames,
            kernel_physical_memory::FrameState::Free,
        );
        memory_regions.push(region);
    }

    // Mark frames allocated by stage1
    let mut stage1_marked = 0;
    for frame in stage1.usable_frames().take(stage_one_next_free) {
        let addr = frame.start_address().as_u64();
        let mut found = false;
        for region in &mut memory_regions {
            if let Some(idx) = region.frame_index(addr) {
                region.frames_mut()[idx] = kernel_physical_memory::FrameState::Allocated;
                stage1_marked += 1;
                found = true;
                break;
            }
        }
        if !found {
            warn!("Stage 1 frame {:#x} not found in any memory region!", addr);
        }
    }
    info!("Marked {} Stage 1 frames as allocated", stage1_marked);

    // Also mark all reserved regions as allocated
    let mut reserved_marked = 0;
    let reserved = RESERVED_REGIONS.lock();
    for i in 0..reserved.count {
        let res = &reserved.regions[i];
        let start_addr = res.base;
        let end_addr = res.base + res.length;
        
        // Align to 4KiB
        let start_addr = (start_addr / 4096) * 4096;
        let end_addr = ((end_addr + 4095) / 4096) * 4096;

        for addr in (start_addr..end_addr).step_by(4096) {
            for region in &mut memory_regions {
                if let Some(idx) = region.frame_index(addr) {
                    if region.frames()[idx] == kernel_physical_memory::FrameState::Free {
                        region.frames_mut()[idx] = kernel_physical_memory::FrameState::Allocated;
                        reserved_marked += 1;
                    }
                    break;
                }
            }
        }
    }
    info!("Marked {} additional reserved frames as allocated", reserved_marked);

    let mut free_count = 0;
    for region in &memory_regions {
        for &state in region.frames() {
            if state == FrameState::Free {
                free_count += 1;
            }
        }
    }
    info!("Total free frames available for Stage 2: {}", free_count);

    if free_count == 0 {
        error!("NO FREE PHYSICAL MEMORY FOR STAGE 2!");
    }

    let bitmap_allocator = PhysicalMemoryManager::new(memory_regions);
    let mut stage2 = MultiStageAllocator::Stage2(bitmap_allocator);
    swap(&mut *guard, &mut stage2);
}

enum MultiStageAllocator {
    Stage1(PhysicalBumpAllocator),
    Stage2(PhysicalMemoryManager),
}

impl<S: PageSize> FrameAllocator<S> for MultiStageAllocator
where
    PhysicalMemoryManager: PhysicalFrameAllocator<S>,
{
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        let res = match self {
            Self::Stage1(a) => {
                if S::SIZE == Size4KiB::SIZE {
                    // Stage1 allocator (BumpAllocator) works with u64 addresses internally
                    // but we need to cast the result to the generic PhysFrame<S>
                    // Since we checked S::SIZE == 4KiB, this is safe-ish for now.
                    let frame_4k = a.allocate_frame()?;
                    Some(PhysFrame::<S>::from_start_address(PhysAddr::new(frame_4k.start_address().as_u64())).unwrap())
                } else {
                    unimplemented!("can't allocate non-4KiB frames in stage1")
                }
            }
            Self::Stage2(a) => a.allocate_frame(),
        };
        if res.is_none() {
            warn!("out of physical memory");
        }
        res
    }

    fn allocate_frames(&mut self, n: usize) -> Option<PhysFrameRange<S>> {
        match self {
            Self::Stage1(_) => unimplemented!("can't allocate contiguous frames in stage1"),
            Self::Stage2(a) => a.allocate_frames(n),
        }
    }

    fn deallocate_frame(&mut self, _frame: PhysFrame<S>) {
        match self {
            Self::Stage1(_) => unimplemented!("can't deallocate frames in stage1"),
            Self::Stage2(_a) => {
                #[cfg(target_arch = "aarch64")]
                { if _a.deallocate_frame(_frame).is_none() {
                    warn!("Failed to deallocate frame {:#x}", _frame.start_address().as_u64());
                } }
                #[cfg(target_arch = "x86_64")]
                { /* omitted */ }
            }
        }
    }

    fn deallocate_frames(&mut self, _range: PhysFrameRange<S>) {
        match self {
            Self::Stage1(_) => unimplemented!("can't deallocate frames in stage1"),
            Self::Stage2(_a) => {
                #[cfg(target_arch = "aarch64")]
                { 
                    // kernel_physical_memory::PhysFrameRangeInclusive is different from our PhysFrameRange
                    // Need to be careful here if we ever use this.
                    // For now, let's just use the trait's default deallocate_frames or loop.
                }
            }
        }
    }
}

pub trait FrameAllocator<S: PageSize> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>>;
    fn allocate_frames(&mut self, n: usize) -> Option<PhysFrameRange<S>>;
    fn deallocate_frame(&mut self, frame: PhysFrame<S>);
    fn deallocate_frames(&mut self, range: PhysFrameRange<S>);
}

struct PhysicalBumpAllocator {
    regions: &'static [MemoryRegion],
    next_frame: usize,
}

impl PhysicalBumpAllocator {
    fn new(regions: &'static [MemoryRegion]) -> Self {
        Self { regions, next_frame: 0 }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.regions.iter()
            .flat_map(|region| (region.base..(region.base + region.length)).step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
            .filter(|frame| {
                let addr = frame.start_address().as_u64();
                RESERVED_REGIONS.lock().is_reserved(addr) == false
            })
    }

    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next_frame);
        if frame.is_some() { self.next_frame += 1; }
        frame
    }
}
