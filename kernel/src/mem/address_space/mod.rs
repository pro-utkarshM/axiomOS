use core::fmt::{Debug, Formatter};

use conquer_once::spin::OnceCell;
#[cfg(target_arch = "x86_64")]
use limine::memory_map::EntryType;
use log::info;
use mapper::AddressSpaceMapper;
use spin::RwLock;

#[cfg(target_arch = "x86_64")]
use log::{debug, trace};

#[cfg(target_arch = "x86_64")]
use x86_64::instructions::interrupts;
#[cfg(target_arch = "x86_64")]
use x86_64::registers::control::Cr3;
#[cfg(target_arch = "x86_64")]
use x86_64::structures::paging::mapper::{
    FlagUpdateError, MapToError, MappedFrame, MapperAllSizes, PageTableFrameMapping,
    TranslateResult,
};
#[cfg(target_arch = "x86_64")]
use x86_64::structures::paging::{
    MappedPageTable, Mapper, OffsetPageTable, PageTable, RecursivePageTable, Translate,
};

use crate::arch::types::{
    Page, PageRangeInclusive, PageSize, PageTableFlags, PhysAddr, PhysFrame, VirtAddr,
};

#[cfg(target_arch = "x86_64")]
use crate::arch::types::Size4KiB;

#[cfg(target_arch = "aarch64")]
pub mod aarch64 {
    use super::*;

    pub fn aarch64_init() -> (VirtAddr, PhysFrame) {
        let phys = crate::arch::aarch64::mm::kernel_page_table_phys();
        let frame = PhysFrame::containing_address(PhysAddr::new(phys as u64));
        // The bootstrap page tables are statically allocated in the kernel image.
        // Their virtual address is the same as their physical address during early boot (identity mapped),
        // and they remain accessible in the higher half after MMU is enabled.
        let vaddr = VirtAddr::new(crate::arch::aarch64::mem::phys_to_virt(phys) as u64);

        (vaddr, frame)
    }
}

#[cfg(target_arch = "x86_64")]
use crate::limine::{HHDM_REQUEST, KERNEL_ADDRESS_REQUEST, MEMORY_MAP_REQUEST};
#[cfg(target_arch = "x86_64")]
use crate::mem::phys::PhysicalMemory;
#[cfg(target_arch = "x86_64")]
use crate::mem::virt::{VirtualMemoryAllocator, VirtualMemoryHigherHalf};
#[cfg(target_arch = "x86_64")]
use crate::{U64Ext, UsizeExt};

mod mapper;

static KERNEL_ADDRESS_SPACE: OnceCell<AddressSpace> = OnceCell::uninit();
#[cfg(target_arch = "x86_64")]
pub static RECURSIVE_INDEX: OnceCell<usize> = OnceCell::uninit();

pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        let (pt_vaddr, pt_frame) = make_mapping_recursive();
        // SAFETY: We have just created the page table frame and mapped it recursively.
        // pt_vaddr and pt_frame are valid and compatible.
        let address_space = unsafe { AddressSpace::create_from(pt_frame, pt_vaddr) };
        info!(
            "Initialized kernel address space with frame: {:?}",
            address_space.level4_frame
        );
        KERNEL_ADDRESS_SPACE.init_once(|| address_space);
    }

    #[cfg(target_arch = "aarch64")]
    {
        let (pt_vaddr, pt_frame) = aarch64::aarch64_init();
        let address_space = unsafe { AddressSpace::create_from(pt_frame, pt_vaddr) };
        info!(
            "Initialized kernel address space with frame: {:?}",
            address_space.level0_frame
        );
        KERNEL_ADDRESS_SPACE.init_once(|| address_space);
    }
}

#[cfg(target_arch = "x86_64")]
fn make_mapping_recursive() -> (VirtAddr, PhysFrame) {
    let hhdm_offset = HHDM_REQUEST
        .get_response()
        .expect("should have a HHDM response")
        .offset();

    let (level_4_table, level_4_table_frame) = {
        let frame = PhysicalMemory::allocate_frame().unwrap();
        // SAFETY: We allocated a fresh frame, so we have exclusive access.
        // The HHDM offset allows us to access physical memory at this virtual address.
        let pt = unsafe {
            &mut *VirtAddr::new(frame.start_address().as_u64() + hhdm_offset)
                .as_mut_ptr::<PageTable>()
        };
        pt.zero();
        (pt, frame)
    };

    // SAFETY: We are creating an OffsetPageTable using the current active page table (Cr3).
    // The HHDM offset is valid as guaranteed by the bootloader/limine.
    let mut current_pt = unsafe {
        OffsetPageTable::new(
            &mut *VirtAddr::new(Cr3::read().0.start_address().as_u64() + hhdm_offset)
                .as_mut_ptr::<PageTable>(),
            VirtAddr::new(hhdm_offset),
        )
    };

    let mut new_pt = {
        struct Offset(u64);
        // SAFETY: The frame_to_pointer implementation uses the fixed HHDM offset
        // which maps all physical memory.
        unsafe impl PageTableFrameMapping for Offset {
            fn frame_to_pointer(&self, frame: PhysFrame) -> *mut PageTable {
                VirtAddr::new(frame.start_address().as_u64() + self.0).as_mut_ptr::<PageTable>()
            }
        }
        // SAFETY: Creating a MappedPageTable with our new level 4 table and the valid offset mapper.
        unsafe { MappedPageTable::new(level_4_table, Offset(hhdm_offset)) }
    };

    let kernel_addr = KERNEL_ADDRESS_REQUEST
        .get_response()
        .unwrap()
        .virtual_base();
    assert_eq!(
        kernel_addr, 0xffff_ffff_8000_0000,
        "kernel address should be 0xffff_ffff_8000_0000, if it isn't, either check the linker file or you know what you're doing"
    );

    info!("remapping kernel");
    remap(
        &mut current_pt,
        &mut new_pt,
        VirtAddr::new(kernel_addr),
        usize::MAX - kernel_addr.into_usize() + 1, // remap from the kernel base until the end of the address space
    );

    MEMORY_MAP_REQUEST
        .get_response()
        .unwrap()
        .entries()
        .iter()
        .filter(|e| e.entry_type == EntryType::EXECUTABLE_AND_MODULES)
        .for_each(|e| {
            info!(
                "remapping module of size ~{}MiB ({} bytes) at virt={:p}",
                e.length / 1024 / 1024,
                e.length,
                VirtAddr::new(e.base + hhdm_offset),
            );
            remap(
                &mut current_pt,
                &mut new_pt,
                VirtAddr::new(e.base + hhdm_offset),
                e.length.into_usize(),
            );
        });

    MEMORY_MAP_REQUEST
        .get_response()
        .unwrap()
        .entries()
        .iter()
        .filter(|e| e.entry_type == EntryType::BOOTLOADER_RECLAIMABLE)
        .for_each(|e| {
            remap(
                &mut current_pt,
                &mut new_pt,
                VirtAddr::new(e.base + hhdm_offset),
                e.length.into_usize(),
            );
        });

    let recursive_index = (0..512)
        .rposition(|p| level_4_table[p].is_unused())
        .expect("should have an unused index in the level 4 table");
    level_4_table[recursive_index].set_frame(
        level_4_table_frame,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
    );
    let vaddr = recursive_index_to_virtual_address(recursive_index);
    debug!("recursive index: {recursive_index:?}, vaddr: {vaddr:p}");
    RECURSIVE_INDEX.init_once(|| recursive_index);

    level_4_table
        .iter_mut()
        .skip(256)
        .filter(|e| e.is_unused())
        .for_each(|e| {
            e.set_frame(
                PhysicalMemory::allocate_frame().unwrap(),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            );
        });

    info!("switching to recursive mapping");
    // SAFETY: We have constructed a valid level 4 page table with all necessary mappings
    // (kernel, HHDM, modules) and a recursive mapping. We are switching CR3 to use it.
    unsafe {
        let cr3_flags = Cr3::read().1;
        Cr3::write(level_4_table_frame, cr3_flags);
    }

    (vaddr, level_4_table_frame)
}

#[cfg(target_arch = "x86_64")]
fn remap(
    current_pt: &mut OffsetPageTable,
    new_pt: &mut impl MapperAllSizes,
    start_vaddr: VirtAddr,
    len: usize,
) {
    let mut current_addr = start_vaddr;

    while current_addr.as_u64() <= start_vaddr.as_u64() - 1 + len as u64 {
        let result = current_pt.translate(current_addr);
        let TranslateResult::Mapped {
            frame,
            offset,
            flags,
        } = result
        else {
            break;
        };

        let flags = flags.intersection(
            PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE
                | PageTableFlags::HUGE_PAGE,
        );

        if offset != 0 {
            // There are cases where limine maps huge pages across borders of memory regions
            // in the HHDM for example, the last pages of a 'usable' section and the first
            // pages of a 'bootloader reclaimable' section could be mapped to the same 2MiB or 1GiB
            // huge frame. We need to handle this accordingly.

            let mut flags = flags;
            flags.remove(PageTableFlags::HUGE_PAGE);

            let MappedFrame::Size2MiB(f) = frame else {
                todo!("support huge pages crossing region borders");
            };

            trace!(
                "breaking up cross-region huge page ({:p} offset {:x})",
                f.start_address(),
                offset
            );

            let mut off = 0;
            while (current_addr + off).as_u64() < (start_vaddr.as_u64() + len.into_u64())
                && (offset + off < frame.size())
            {
                let page = Page::<Size4KiB>::containing_address(current_addr + off);
                let f1 = PhysFrame::containing_address(f.start_address() + offset + off);
                // SAFETY: We are splitting a huge page into 4KiB pages. The target frames exist.
                // We are updating the new page table which is not yet active.
                unsafe {
                    let _ = new_pt.map_to(page, f1, flags, &mut PhysicalMemory).unwrap();
                }
                off += page.size();
            }
        } else {
            // SAFETY: We are mapping pages in the new page table. The frames are valid as they
            // were obtained from translation of the current page table.
            unsafe {
                match frame {
                    MappedFrame::Size4KiB(f) => {
                        let _ = new_pt
                            .map_to(
                                Page::containing_address(current_addr),
                                f,
                                flags,
                                &mut PhysicalMemory,
                            )
                            .unwrap();
                    }
                    MappedFrame::Size2MiB(f) => {
                        let _ = new_pt
                            .map_to(
                                Page::containing_address(current_addr),
                                f,
                                flags,
                                &mut PhysicalMemory,
                            )
                            .unwrap();
                    }
                    MappedFrame::Size1GiB(f) => {
                        let _ = new_pt
                            .map_to(
                                Page::containing_address(current_addr),
                                f,
                                flags,
                                &mut PhysicalMemory,
                            )
                            .unwrap();
                    }
                }
            }
        }
        current_addr += frame.size() - offset;
    }
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub const fn recursive_index_to_virtual_address(recursive_index: usize) -> VirtAddr {
    let i = recursive_index as u64;
    let addr = (i << 39) | (i << 30) | (i << 21) | (i << 12);

    let addr = sign_extend_vaddr(addr);

    VirtAddr::new(addr)
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub const fn virt_addr_from_page_table_indices(indices: [u16; 4], offset: u64) -> VirtAddr {
    let addr = ((indices[0] as u64) << 39)
        | ((indices[1] as u64) << 30)
        | ((indices[2] as u64) << 21)
        | ((indices[3] as u64) << 12)
        | (offset & ((1 << 12) - 1));
    VirtAddr::new(sign_extend_vaddr(addr))
}

#[cfg(target_arch = "x86_64")]
#[must_use]
pub const fn sign_extend_vaddr(vaddr: u64) -> u64 {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
    let result = ((vaddr << 16) as i64 >> 16) as u64; // only works for 48-bit addresses
    result
}

pub struct AddressSpace {
    #[cfg(target_arch = "x86_64")]
    level4_frame: PhysFrame,
    #[cfg(target_arch = "aarch64")]
    level0_frame: crate::arch::aarch64::phys::PhysFrame,
    inner: RwLock<AddressSpaceMapper>,
}

impl Debug for AddressSpace {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut ds = f.debug_struct("AddressSpace");

        #[cfg(target_arch = "x86_64")]
        ds.field("level4_frame", &self.level4_frame);
        #[cfg(target_arch = "aarch64")]
        ds.field("level0_frame", &self.level0_frame);

        ds.field("active", &self.inner.read().is_active())
            .finish_non_exhaustive()
    }
}

impl !Default for AddressSpace {}

impl AddressSpace {
    /// # Panics
    /// Panics if the kernel address space is not initialized yet.
    pub fn kernel() -> &'static Self {
        KERNEL_ADDRESS_SPACE
            .get()
            .expect("address space not initialized")
    }

    /// # Safety
    /// The level4_frame must be a valid physical frame containing a top-level page table.
    /// The level4_vaddr must be the virtual address where that frame is mapped.
    #[cfg(target_arch = "x86_64")]
    unsafe fn create_from(level4_frame: PhysFrame, level4_vaddr: VirtAddr) -> Self {
        Self {
            level4_frame,
            inner: RwLock::new(AddressSpaceMapper::new(level4_frame, level4_vaddr)),
        }
    }

    /// # Safety
    /// The level0_frame must be a valid physical frame containing a top-level page table.
    /// The level0_vaddr must be the virtual address where that frame is mapped.
    #[cfg(target_arch = "aarch64")]
    unsafe fn create_from(
        level0_frame: crate::arch::aarch64::phys::PhysFrame,
        level0_vaddr: crate::arch::types::VirtAddr,
    ) -> Self {
        Self {
            level0_frame,
            inner: RwLock::new(AddressSpaceMapper::new(level0_frame, level0_vaddr)),
        }
    }

    /// # Panics
    /// This function panics if not enough physical or virtual memory is available to create
    /// a new address space, or if something goes wrong during mapping of addresses.
    #[must_use]
    pub fn new() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let new_frame = PhysicalMemory::allocate_frame().unwrap();
            let new_pt_segment = VirtualMemoryHigherHalf.reserve(1).unwrap();
            let old_pt_segment = VirtualMemoryHigherHalf.reserve(1).unwrap();

            let old_pt_page = Page::containing_address(old_pt_segment.start);
            let new_pt_page = Page::containing_address(new_pt_segment.start);

            Self::kernel().with_active(|kernel_as| {
                kernel_as
                    .map::<Size4KiB>(
                        old_pt_page,
                        kernel_as.level4_frame,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                    )
                    .unwrap();

                kernel_as
                    .map::<Size4KiB>(
                        new_pt_page,
                        new_frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE,
                    )
                    .unwrap();

                // SAFETY: We have reserved segments in higher-half virtual memory.
                // We are casting the pointers to PageTable which matches the underlying data structure.
                // These pointers are valid because we just mapped the frames in the active (kernel) address space.
                let new_page_table = unsafe { &mut *new_pt_segment.start.as_mut_ptr::<PageTable>() };
                // SAFETY: Same as above.
                let old_page_table = unsafe { &*old_pt_segment.start.as_mut_ptr::<PageTable>() };

                new_page_table.zero();
                new_page_table
                    .iter_mut()
                    .zip(old_page_table.iter())
                    .skip(256)
                    .for_each(|(new_entry, old_entry)| {
                        *new_entry = old_entry.clone();
                    });
                let recursive_index = *RECURSIVE_INDEX.get().unwrap();
                new_page_table[recursive_index].set_frame(
                    new_frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
                );

                kernel_as
                    .unmap(old_pt_page)
                    .expect("page should be mapped");
                kernel_as
                    .unmap(new_pt_page)
                    .expect("page should be mapped");
            });

            // SAFETY: We have initialized the new page table frame and mapped it.
            // We reuse the existing recursive mapping vaddr since we copied the recursive entry.
            unsafe { Self::create_from(new_frame, Self::kernel().inner.read().level4_vaddr) }
        }

        #[cfg(target_arch = "aarch64")]
        {
            let l0_phys_ptr = crate::arch::aarch64::mm::create_user_address_space()
                .expect("failed to create user address space");
            let l0_phys = l0_phys_ptr as usize;
            let frame = crate::arch::aarch64::phys::PhysFrame::containing_address(
                crate::arch::types::PhysAddr::new(l0_phys as u64),
            );
            // Convert physical address to virtual address using the direct map (HHDM equivalent)
            let vaddr = crate::arch::types::VirtAddr::new(
                crate::arch::aarch64::mem::phys_to_virt(l0_phys) as u64,
            );
            unsafe { Self::create_from(frame, vaddr) }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn cr3_value(&self) -> usize {
        self.level4_frame.start_address().as_u64().into_usize()
    }

    #[cfg(target_arch = "aarch64")]
    pub fn ttbr0_value(&self) -> usize {
        self.level0_frame.addr() as usize
    }

    pub fn with_active<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Self) -> R,
    {
        #[cfg(target_arch = "x86_64")]
        {
            let current_cr3 = Cr3::read();
            let current_frame = current_cr3.0;
            let current_flags = current_cr3.1;

            if current_frame == self.level4_frame {
                return f(self);
            }

            interrupts::without_interrupts(|| {
                // SAFETY: We are temporarily switching to this address space to perform operations on it.
                // We ensure that we switch back to the original address space afterwards.
                // Since kernel mappings are shared, this is safe for kernel execution.
                unsafe {
                    Cr3::write(self.level4_frame, current_flags);
                }

                let result = f(self);

                // SAFETY: Restoring the original address space.
                unsafe {
                    Cr3::write(current_frame, current_flags);
                }

                result
            })
        }

        #[cfg(target_arch = "aarch64")]
        {
            let current_ttbr0 = crate::arch::aarch64::paging::get_ttbr0();
            let current_ttbr1 = crate::arch::aarch64::paging::get_ttbr1();
            let target_phys = self.level0_frame.addr() as usize;

            // If the target address space is already active in either TTBR0 or TTBR1,
            // we don't need to switch. TTBR1 is used for the kernel address space.
            if target_phys == current_ttbr0 || target_phys == current_ttbr1 {
                return f(self);
            }

            // SAFETY: Switching TTBR0 to the target address space.
            // This is safe because kernel mappings are shared across all address spaces (via TTBR1).
            unsafe {
                crate::arch::aarch64::paging::set_ttbr0(target_phys);
            }

            let result = f(self);

            // SAFETY: Restoring the original TTBR0.
            unsafe {
                crate::arch::aarch64::paging::set_ttbr0(current_ttbr0);
            }

            result
        }
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.inner.read().is_active()
    }

    /// # Errors
    /// Returns an error if the page is already mapped or flags are invalid.
    #[allow(dead_code)]
    #[cfg(target_arch = "x86_64")]
    pub fn map<S: PageSize>(
        &self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<S>>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        self.inner.write().map(page, frame, flags)
    }

    /// # Errors
    /// Returns an error if the pages are already mapped or flags are invalid.
    #[cfg(target_arch = "x86_64")]
    pub fn map_range<S: PageSize>(
        &self,
        pages: impl Into<PageRangeInclusive<S>>,
        frames: impl Iterator<Item = PhysFrame<S>>,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<S>>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        self.inner.write().map_range(pages.into(), frames, flags)
    }

    #[cfg(target_arch = "x86_64")]
    pub fn unmap<S: PageSize>(&self, page: Page<S>) -> Option<PhysFrame<S>>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        self.inner.write().unmap(page)
    }

    #[cfg(target_arch = "x86_64")]
    pub fn unmap_range<S: PageSize>(
        &self,
        pages: impl Into<PageRangeInclusive<S>>,
        callback: impl Fn(PhysFrame<S>),
    ) where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        self.inner.write().unmap_range(pages.into(), callback);
    }

    /// # Errors
    /// Returns an error if the page is not mapped or flags are invalid.
    #[cfg(target_arch = "x86_64")]
    pub fn remap<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &self,
        page: Page<S>,
        f: F,
    ) -> Result<(), FlagUpdateError>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        self.inner.write().remap(page, &f)
    }

    /// # Errors
    /// Returns an error if the pages are not mapped or flags are invalid.
    #[cfg(target_arch = "x86_64")]
    pub fn remap_range<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &self,
        pages: impl Into<PageRangeInclusive<S>>,
        f: F,
    ) -> Result<(), FlagUpdateError>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        self.inner.write().remap_range(pages.into(), &f)
    }

    #[allow(dead_code)]
    #[cfg(target_arch = "x86_64")]
    pub fn translate(&self, vaddr: VirtAddr) -> Option<PhysAddr> {
        self.inner.read().translate(vaddr)
    }

    #[allow(dead_code)]
    #[cfg(target_arch = "aarch64")]
    pub fn translate(&self, vaddr: VirtAddr) -> Option<PhysAddr> {
        self.inner.read().translate(vaddr)
    }

    #[cfg(target_arch = "aarch64")]
    pub fn map<S: PageSize>(
        &self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
    ) -> Result<(), &'static str> {
        self.inner.write().map(page, frame, flags)
    }

    #[cfg(target_arch = "aarch64")]
    pub fn map_range<S: PageSize>(
        &self,
        pages: impl Into<PageRangeInclusive<S>>,
        frames: impl Iterator<Item = PhysFrame<S>>,
        flags: PageTableFlags,
    ) -> Result<(), &'static str> {
        self.inner.write().map_range(pages.into(), frames, flags)
    }

    #[cfg(target_arch = "aarch64")]
    pub fn unmap<S: PageSize>(&self, page: Page<S>) -> Option<PhysFrame<S>> {
        self.inner.write().unmap(page)
    }

    #[cfg(target_arch = "aarch64")]
    pub fn unmap_range<S: PageSize>(
        &self,
        pages: impl Into<PageRangeInclusive<S>>,
        callback: impl Fn(PhysFrame<S>),
    ) {
        self.inner.write().unmap_range(pages.into(), callback);
    }

    #[cfg(target_arch = "aarch64")]
    pub fn remap<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &self,
        page: Page<S>,
        f: F,
    ) -> Result<(), &'static str> {
        self.inner.write().remap(page, &f)
    }

    #[cfg(target_arch = "aarch64")]
    pub fn remap_range<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &self,
        pages: impl Into<PageRangeInclusive<S>>,
        f: F,
    ) -> Result<(), &'static str> {
        self.inner.write().remap_range(pages.into(), &f)
    }
}
