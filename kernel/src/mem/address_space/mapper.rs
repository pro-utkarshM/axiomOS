#[cfg(target_arch = "x86_64")]
use x86_64::registers::control::Cr3;
#[cfg(target_arch = "x86_64")]
use x86_64::structures::paging::mapper::{FlagUpdateError, MapToError, TranslateResult};
#[cfg(target_arch = "x86_64")]
use x86_64::structures::paging::{Mapper, PageTable, RecursivePageTable, Translate};

#[cfg(target_arch = "aarch64")]
use crate::arch::aarch64::paging::PageTableWalker;

use crate::arch::types::{
    Page, PageRangeInclusive, PageSize, PageTableFlags, PhysAddr, PhysFrame, VirtAddr,
};

#[cfg(target_arch = "x86_64")]
use crate::mem::phys::PhysicalMemory;

#[derive(Debug)]
pub struct AddressSpaceMapper {
    #[cfg(target_arch = "x86_64")]
    level4_frame: PhysFrame,
    #[cfg(target_arch = "x86_64")]
    pub(crate) level4_vaddr: VirtAddr,
    #[cfg(target_arch = "x86_64")]
    page_table: RecursivePageTable<'static>,

    #[cfg(target_arch = "aarch64")]
    level0_frame: PhysFrame,
    #[cfg(target_arch = "aarch64")]
    pub(crate) level0_vaddr: VirtAddr,
}

impl AddressSpaceMapper {
    #[cfg(target_arch = "x86_64")]
    pub fn new(level4_frame: PhysFrame, level4_vaddr: VirtAddr) -> Self {
        let page_table = {
            // SAFETY: The caller ensures that level4_vaddr points to a valid PageTable
            // and that we have exclusive access (or appropriate synchronization) to it.
            let pt = unsafe { &mut *level4_vaddr.as_mut_ptr::<PageTable>() };
            RecursivePageTable::new(pt).expect("should be a valid recursive page table")
        };

        Self {
            level4_frame,
            level4_vaddr,
            page_table,
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn new(level0_frame: PhysFrame, level0_vaddr: VirtAddr) -> Self {
        Self {
            level0_frame,
            level0_vaddr,
        }
    }

    pub fn is_active(&self) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            self.level4_frame == Cr3::read().0
        }

        #[cfg(target_arch = "aarch64")]
        {
            self.level0_frame.addr() == crate::arch::aarch64::paging::get_ttbr0() as u64
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn map<S: PageSize>(
        &mut self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<S>>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        assert!(self.is_active());

        #[cfg(debug_assertions)]
        {
            if !flags.contains(PageTableFlags::PRESENT) {
                ::log::warn!(
                    "mapping {:p} to {:p} without PRESENT flag",
                    page.start_address(),
                    frame.start_address()
                );
            }
        }

        // SAFETY: We hold a mutable reference to the mapper, ensuring exclusive access.
        // The frame allocator (PhysicalMemory) is thread-safe.
        unsafe {
            self.page_table
                .map_to(page, frame, flags, &mut PhysicalMemory)?
                .flush();
        }

        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    pub fn map_range<S: PageSize>(
        &mut self,
        pages: PageRangeInclusive<S>,
        frames: impl Iterator<Item = PhysFrame<S>>,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<S>>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        assert!(self.is_active());

        let mut frames = frames.into_iter();

        for page in pages {
            let frame = frames.next().ok_or(MapToError::FrameAllocationFailed)?;
            self.map(page, frame, flags)?;
        }

        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    pub fn unmap<S: PageSize>(&mut self, page: Page<S>) -> Option<PhysFrame<S>>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        assert!(self.is_active());

        if let Ok((frame, flusher)) = self.page_table.unmap(page) {
            flusher.flush();
            Some(frame)
        } else {
            None
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn unmap_range<S: PageSize>(
        &mut self,
        pages: PageRangeInclusive<S>,
        callback: impl Fn(PhysFrame<S>),
    ) where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        assert!(self.is_active());

        for page in pages {
            self.unmap(page).map(&callback);
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn remap<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &mut self,
        page: Page<S>,
        f: &F,
    ) -> Result<(), FlagUpdateError>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        assert!(self.is_active());

        let TranslateResult::Mapped {
            frame: _,
            offset: _,
            flags,
        } = self.page_table.translate(page.start_address())
        else {
            return Err(FlagUpdateError::PageNotMapped);
        };
        // SAFETY: We checked that the page is mapped. We hold exclusive access via &mut self.
        let flusher = unsafe { self.page_table.update_flags(page, f(flags)) }?;
        flusher.flush();
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    pub fn remap_range<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &mut self,
        pages: PageRangeInclusive<S>,
        f: &F,
    ) -> Result<(), FlagUpdateError>
    where
        for<'a> RecursivePageTable<'a>: Mapper<S>,
    {
        assert!(self.is_active());

        for page in pages {
            self.remap(page, &f)?;
        }
        Ok(())
    }

    #[cfg(target_arch = "aarch64")]
    pub fn map<S: PageSize>(
        &mut self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
    ) -> Result<(), &'static str> {
        let mut walker = unsafe { PageTableWalker::new(self.level0_vaddr.as_mut_ptr()) };
        walker.map_page(
            page.start_address().as_usize(),
            frame.start_address().as_u64() as usize,
            flags.to_pte_bits(),
        )
    }

    #[cfg(target_arch = "aarch64")]
    pub fn map_range<S: PageSize>(
        &mut self,
        pages: PageRangeInclusive<S>,
        frames: impl Iterator<Item = PhysFrame<S>>,
        flags: PageTableFlags,
    ) -> Result<(), &'static str> {
        let mut frames = frames.into_iter();
        for page in pages {
            let frame = frames.next().ok_or("Not enough frames for range")?;
            self.map(page, frame, flags)?;
        }
        Ok(())
    }

    #[cfg(target_arch = "aarch64")]
    pub fn unmap<S: PageSize>(&mut self, page: Page<S>) -> Option<PhysFrame<S>> {
        let mut walker = unsafe { PageTableWalker::new(self.level0_vaddr.as_mut_ptr()) };
        walker
            .unmap_page(page.start_address().as_usize())
            .ok()
            .map(|phys| PhysFrame::containing_address(PhysAddr::new(phys as u64)))
    }

    #[cfg(target_arch = "aarch64")]
    pub fn unmap_range<S: PageSize>(
        &mut self,
        pages: PageRangeInclusive<S>,
        callback: impl Fn(PhysFrame<S>),
    ) {
        for page in pages {
            if let Some(frame) = self.unmap(page) {
                callback(frame);
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn remap<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &mut self,
        page: Page<S>,
        f: &F,
    ) -> Result<(), &'static str> {
        let mut walker = unsafe { PageTableWalker::new(self.level0_vaddr.as_mut_ptr()) };
        let vaddr = page.start_address().as_usize();
        let (_phys, raw_flags) = walker.translate_full(vaddr).ok_or("Page not mapped")?;

        let old_flags = PageTableFlags::from_pte_bits(raw_flags);
        let new_flags = f(old_flags);

        // AArch64 doesn't have an easy "update flags" without unmap/map in the current walker
        // but we can just map again with different flags if the walker supports it,
        // or unmap and map.
        let phys = walker.unmap_page(vaddr)?;
        walker.map_page(vaddr, phys, new_flags.to_pte_bits())
    }

    #[cfg(target_arch = "aarch64")]
    pub fn remap_range<S: PageSize, F: Fn(PageTableFlags) -> PageTableFlags>(
        &mut self,
        pages: PageRangeInclusive<S>,
        f: &F,
    ) -> Result<(), &'static str> {
        for page in pages {
            self.remap(page, f)?;
        }
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    pub fn translate(&self, vaddr: VirtAddr) -> Option<PhysAddr> {
        self.page_table.translate_addr(vaddr)
    }

    #[cfg(target_arch = "aarch64")]
    pub fn translate(&self, vaddr: VirtAddr) -> Option<PhysAddr> {
        let walker = unsafe { PageTableWalker::new(self.level0_vaddr.as_mut_ptr()) };
        walker
            .translate(vaddr.as_usize())
            .map(|phys| PhysAddr::new(phys as u64))
    }
}
