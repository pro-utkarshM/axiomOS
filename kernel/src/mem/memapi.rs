use alloc::sync::Arc;
use core::alloc::Layout;
use core::fmt::{Debug, Formatter};
use core::marker::PhantomData;
use core::ops::Deref;
use core::slice::{from_raw_parts, from_raw_parts_mut};

use kernel_memapi::{Allocation, Guarded, Location, MemoryApi, UserAccessible, WritableAllocation};
use kernel_virtual_memory::Segment;
use crate::arch::types::{VirtAddr, PageSize, Size4KiB, PageTableFlags};

use crate::mcore::mtask::process::Process;
use crate::mem::phys::PhysicalMemory;
use crate::mem::virt::{OwnedSegment, VirtualMemoryAllocator};
use crate::{U64Ext, UsizeExt};

#[derive(Clone)]
pub struct LowerHalfMemoryApi {
    process: Arc<Process>,
}

impl LowerHalfMemoryApi {
    pub fn new(process: Arc<Process>) -> Self {
        Self { process }
    }
}

impl MemoryApi for LowerHalfMemoryApi {
    type ReadonlyAllocation = LowerHalfAllocation<Readonly>;
    type WritableAllocation = LowerHalfAllocation<Writable>;
    type ExecutableAllocation = LowerHalfAllocation<Executable>;

    fn allocate(
        &mut self,
        location: Location,
        layout: Layout,
        user_accessible: UserAccessible,
        guarded: Guarded,
    ) -> Option<Self::WritableAllocation> {
        assert!(layout.align() <= Size4KiB::SIZE.into_usize());

        let num_pages = layout.size().div_ceil(Size4KiB::SIZE.into_usize())
            + match guarded {
                Guarded::Yes => 2, // Reserve two extra pages for guard pages
                Guarded::No => 0,
            };

        let (start, segment) = match location {
            Location::Anywhere => {
                let segment = self.process.vmm().reserve(num_pages)?;
                (None, segment)
            }
            Location::Fixed(v) => {
                let v = VirtAddr::new(v);

                // We don't enforce strict alignment check here because ELF segments might not be page-aligned.
                // The logic below ensures we map the containing pages.
                if !v.is_aligned(layout.align() as u64) {
                    log::warn!("LowerHalfMemoryApi: Fixed location {:p} is not aligned to {}, but proceeding by mapping containing pages.", v.as_ptr::<()>(), layout.align());
                }

                let aligned_start_addr = v.align_down(Size4KiB::SIZE)
                    - match guarded {
                        Guarded::Yes => Size4KiB::SIZE,
                        Guarded::No => 0,
                    };
                let aligned_end_addr = (v + layout.size().into_u64()).align_up(Size4KiB::SIZE)
                    + match guarded {
                        Guarded::Yes => Size4KiB::SIZE,
                        Guarded::No => 0,
                    };
                let segment = Segment::new(
                    kernel_virtual_memory::VirtAddr::new(aligned_start_addr.as_u64()),
                    aligned_end_addr.as_u64() - aligned_start_addr.as_u64(),
                );
                let vmm = self.process.vmm();
                let segment = vmm.mark_as_reserved(segment).ok()?;
                (Some(v), segment)
            }
        };

        let mapped_segment = match guarded {
            Guarded::Yes => Segment::new(
                segment.start + Size4KiB::SIZE,
                segment.len - (2 * Size4KiB::SIZE),
            ),
            Guarded::No => *segment,
        };

        self.process
            .with_address_space(|as_| as_.map_range::<Size4KiB>(
                &mapped_segment,
                PhysicalMemory::allocate_frames_non_contiguous(),
                PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE
                    | if user_accessible == UserAccessible::Yes {
                        PageTableFlags::USER_ACCESSIBLE
                    } else {
                        PageTableFlags::empty()
                    },
            ))
            .ok()?;

        let start = start.unwrap_or(mapped_segment.start);
        Some(LowerHalfAllocation {
            start,
            layout,
            inner: Inner {
                process: self.process.clone(),
                segment,
                mapped_segment,
            },
            _typ: PhantomData,
        })
    }

    fn make_executable(
        &mut self,
        allocation: Self::WritableAllocation,
    ) -> Result<Self::ExecutableAllocation, Self::WritableAllocation> {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            extern "C" {
                fn aarch64_jit_sync_cache(start: usize, len: usize);
            }
            // Sync caches before marking executable to ensure I-cache sees the written instructions
            aarch64_jit_sync_cache(allocation.start().as_u64() as usize, allocation.len());
        }

        let res = self.process.with_address_space(|as_| as_.remap_range::<Size4KiB, _>(
            &*allocation.segment,
            |mut flags: PageTableFlags| {
                flags.remove(PageTableFlags::WRITABLE);
                flags.remove(PageTableFlags::NO_EXECUTE);
                flags
            },
        ));
        if res.is_err() {
            return Err(allocation);
        }

        Ok(LowerHalfAllocation {
            start: allocation.start,
            layout: allocation.layout,
            inner: allocation.inner,
            _typ: PhantomData,
        })
    }

    fn make_writable(
        &mut self,
        allocation: Self::ExecutableAllocation,
    ) -> Result<Self::WritableAllocation, Self::ExecutableAllocation> {
        let res = self.process.with_address_space(|as_| as_.remap_range::<Size4KiB, _>(
            &*allocation.segment,
            |mut flags: PageTableFlags| {
                flags.insert(PageTableFlags::WRITABLE);
                flags.insert(PageTableFlags::NO_EXECUTE);
                flags
            },
        ));
        if res.is_err() {
            return Err(allocation);
        }

        Ok(LowerHalfAllocation {
            start: allocation.start,
            layout: allocation.layout,
            inner: allocation.inner,
            _typ: PhantomData,
        })
    }

    fn make_readonly(
        &mut self,
        allocation: Self::WritableAllocation,
    ) -> Result<Self::ReadonlyAllocation, Self::WritableAllocation> {
        let res = self.process.with_address_space(|as_| as_.remap_range::<Size4KiB, _>(
            &*allocation.segment,
            |mut flags: PageTableFlags| {
                flags.remove(PageTableFlags::WRITABLE);
                flags.insert(PageTableFlags::NO_EXECUTE);
                flags
            },
        ));
        if res.is_err() {
            return Err(allocation);
        }

        Ok(LowerHalfAllocation {
            start: allocation.start,
            layout: allocation.layout,
            inner: allocation.inner,
            _typ: PhantomData,
        })
    }
}

trait Sealed {}
#[allow(private_bounds)]
pub trait AllocationType: Sealed + AllocationFlags {}
#[derive(Debug)]
pub struct Readonly;
impl Sealed for Readonly {}
impl AllocationType for Readonly {}
#[derive(Debug)]
pub struct Writable;
impl Sealed for Writable {}
impl AllocationType for Writable {}
#[derive(Debug)]
pub struct Executable;
impl Sealed for Executable {}
impl AllocationType for Executable {}

pub struct LowerHalfAllocation<T> {
    start: VirtAddr,
    layout: Layout,
    inner: Inner,
    _typ: PhantomData<T>,
}

impl<T: AllocationType> LowerHalfAllocation<T> {
    #[must_use]
    pub fn start(&self) -> VirtAddr {
        self.start
    }

    #[allow(clippy::len_without_is_empty)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.layout.size()
    }

    /// Clones this allocation into another process.
    ///
    /// This allocates new physical memory, copies the content, and maps it into the target
    /// process's address space at the same virtual address.
    pub fn clone_to_process(&self, new_process: Arc<Process>) -> Option<Self> {
        // 1. Reserve the same segment in the new process
        // The segment might need to be "Fixed" location reservation.
        // Our VMM allows reserving a specific segment via mark_as_reserved.

        // We need to construct a new Segment with the same range.
        let new_segment_inner = kernel_virtual_memory::Segment::new(
            self.inner.mapped_segment.start,
            self.inner.mapped_segment.len
        );

        let new_segment = new_process.vmm().mark_as_reserved(new_segment_inner).ok()?;

        // 2. Allocate new physical frames
        // We need to know how many pages.
        let page_count = (self.inner.mapped_segment.len / Size4KiB::SIZE as u64) as usize;

        // Use non-contiguous allocation to be safe against fragmentation,
        // though `allocate` uses `allocate_frames_non_contiguous`.
        // We will collect them into a Vec to iterate for copying and mapping.
        let mut new_frames = alloc::vec::Vec::new();
        for _ in 0..page_count {
            new_frames.push(PhysicalMemory::allocate_frame::<Size4KiB>()?);
        }

        // 3. Copy data
        // We can access self's memory via self.as_ref() (it is mapped in current AS).
        // For new frames, we must use the direct map (phys_to_virt).

        let _src_slice = self.as_ref();
        // We need to copy page by page because new_frames are not contiguous.
        // But `src_slice` is virtually contiguous.

        for (i, frame) in new_frames.iter().enumerate() {
            let _src_offset = i * Size4KiB::SIZE as usize;
            // The allocation might include guard pages.
            // `self.inner.mapped_segment` covers the whole mapped range (including guards?).
            // `self.start` is the start of the *usable* memory.
            // Let's check `allocate` impl.
            // `mapped_segment` is what is mapped in page tables.
            // If Guarded::Yes, mapped_segment is smaller than the VMM segment?
            // In allocate:
            // segment = vmm.reserve(...)
            // mapped_segment = segment (shrunk if guarded)
            // So `mapped_segment` contains only the mapped pages (no guards).
            // So we can blindly copy all pages in `mapped_segment`.

            // Wait, `self.as_ref()` returns `from_raw_parts(self.start... self.layout.size())`.
            // `self.start` might be offset from `mapped_segment.start` if alignment adjustments happened?
            // `allocate` says: `start = start.unwrap_or(mapped_segment.start)`.
            // And `mapped_segment` excludes guard pages.

            // So `mapped_segment` represents the *mapped* pages.
            // We should copy the content of these pages.
            // Is it safe to read from `mapped_segment.start`?
            // Yes, it's mapped in the current process.

            let page_vaddr = self.inner.mapped_segment.start + (i as u64 * Size4KiB::SIZE);
            let src_ptr = page_vaddr.as_ptr::<u8>();

            let dst_paddr = frame.start_address().as_u64();
            let dst_vaddr = crate::mem::phys_to_virt(dst_paddr as usize);
            let dst_ptr = dst_vaddr as *mut u8;

            unsafe {
                core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, Size4KiB::SIZE as usize);
            }

            #[cfg(target_arch = "aarch64")]
            if !T::flags().contains(PageTableFlags::NO_EXECUTE) {
                // Sync caches for the destination page to ensure I-cache sees the written instructions.
                // We use the Kernel VA (dst_vaddr) which maps to the same physical address.
                // Note: We need to declare the external function.
                unsafe {
                    extern "C" {
                        fn aarch64_jit_sync_cache(start: usize, len: usize);
                    }
                    aarch64_jit_sync_cache(dst_vaddr, Size4KiB::SIZE as usize);
                }
            }
        }

        // 4. Map in new process address space
        // We need the same flags as the current mapping.
        // We can't easily get flags from `LowerHalfAllocation` struct (phantom data doesn't help).
        // However, `LowerHalfAllocation` is usually Writable or Executable or Readonly.
        // But `T` is a generic type parameter.
        // We can check `T`? No, specialization is unstable.
        // We should probably look up the flags from the current page table?
        // Or just assume RW for now?
        // If it was Executable, we probably want it to be Writable first, then made Executable?
        // Actually, `fork` usually keeps the same permissions.
        // If we clone an `Executable` allocation, we want the result to be `Executable`.
        // But we just wrote to the physical frames (using direct map), so that's fine.
        // We need to map them as Executable in the new AS if T is Executable.

        // Getting flags from current page table:
        let _sample_page = crate::arch::types::Page::<Size4KiB>::containing_address(self.inner.mapped_segment.start);
        // We can use `process.address_space().translate()` but that gives PhysAddr.
        // We need flags. `visit_user_pages` gives flags but iterates everything.
        // Let's add `get_flags(page)` to AddressSpace?
        // For now, let's assume a default set of flags based on usage, or try to be generic.
        // Ideally `LowerHalfAllocation` would store the flags.
        //
        // Let's use a safe default: PRESENT | USER_ACCESSIBLE.
        // Then we check if we can deduce WRITABLE/NO_EXECUTE.
        // `LowerHalfAllocation` doesn't store state about whether it's currently Writable/Exec.
        // BUT, the type `T` *statically* tells us!
        // `LowerHalfAllocation<Writable>` implies Writable.
        // `LowerHalfAllocation<Executable>` implies Executable.
        // We can't easily match on T.

        // HACK: For now, map as WRITABLE | NO_EXECUTE.
        // If the original was Executable, the caller (Process::fork) might need to "fix" it?
        // Or we can rely on `make_executable` being called later?
        // No, `fork` returns a ready-to-go process.

        // Let's look up the flags from the first page of the mapping.
        // We need to expose a way to get flags.
        // Or we can just blindly copy flags from the source page table?
        // Since we are in the kernel and have access to the current AS, we can walk it.
        // But `AddressSpace` abstracts the walker.

        // Alternative: Just map as RWX for now? No, insecure.
        // Let's assume RW for stack (Writable) and RX for code (Executable).
        // Since this method is generic over T, we can't easily distinguish.

        // Let's add a helper trait `AllocationFlags` implemented for `Writable`, `Executable`, `Readonly`.
        let flags = T::flags() | PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

        new_process.with_address_space(|as_| as_.map_range(
            &self.inner.mapped_segment,
            new_frames.into_iter(),
            flags
        )).ok()?;

        Some(LowerHalfAllocation {
            start: self.start,
            layout: self.layout,
            inner: Inner {
                segment: new_segment,
                mapped_segment: self.inner.mapped_segment,
                process: new_process,
            },
            _typ: PhantomData,
        })
    }
}

pub trait AllocationFlags {
    fn flags() -> PageTableFlags;
}

impl AllocationFlags for Writable {
    fn flags() -> PageTableFlags {
        PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE
    }
}

impl AllocationFlags for Readonly {
    fn flags() -> PageTableFlags {
        PageTableFlags::NO_EXECUTE
    }
}

impl AllocationFlags for Executable {
    fn flags() -> PageTableFlags {
        PageTableFlags::empty() // Executable (no NO_EXECUTE) and ReadOnly (no WRITABLE)
    }
}

pub struct Inner {
    segment: OwnedSegment<'static>,
    mapped_segment: Segment,
    process: Arc<Process>,
}

impl<T: AllocationType> Deref for LowerHalfAllocation<T> {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: AllocationType> Debug for LowerHalfAllocation<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LowerHalfAllocation")
            .field("process_id", &self.process.pid())
            .field("segment", &self.segment)
            .field("typ", &self._typ)
            .finish_non_exhaustive()
    }
}

impl<T: AllocationType> AsRef<[u8]> for LowerHalfAllocation<T> {
    fn as_ref(&self) -> &[u8] {
        let ptr = self.start.as_ptr();
        // SAFETY: self.start points to the start of the allocation, and self.layout.size()
        // is the size of the allocation. The allocation is valid for the lifetime of self.
        unsafe { from_raw_parts(ptr, self.layout.size()) }
    }
}

impl<T: AllocationType> Allocation for LowerHalfAllocation<T> {
    fn layout(&self) -> Layout {
        self.layout
    }
}

impl AsMut<[u8]> for LowerHalfAllocation<Writable> {
    fn as_mut(&mut self) -> &mut [u8] {
        let ptr = self.start.as_mut_ptr();
        // SAFETY: self.start points to the start of the allocation, and self.layout.size()
        // is the size of the allocation. We have exclusive access via &mut self.
        unsafe { from_raw_parts_mut(ptr, self.layout.size()) }
    }
}

impl WritableAllocation for LowerHalfAllocation<Writable> {}

impl Drop for Inner {
    fn drop(&mut self) {
        self.process
            .with_address_space(|as_| as_.unmap_range::<Size4KiB>(&self.mapped_segment, PhysicalMemory::deallocate_frame));
    }
}
