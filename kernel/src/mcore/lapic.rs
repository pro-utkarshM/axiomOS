use core::ops::{Deref, DerefMut};

use x2apic::lapic::{xapic_base, LocalApicBuilder, TimerDivide, TimerMode};
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

use crate::arch::idt::InterruptIndex;
use crate::mem::address_space::AddressSpace;
use crate::mem::virt::{OwnedSegment, VirtualMemoryAllocator, VirtualMemoryHigherHalf};

#[derive(Debug)]
pub struct Lapic {
    _segment: OwnedSegment<'static>,
    inner: x2apic::lapic::LocalApic,
}

impl Deref for Lapic {
    type Target = x2apic::lapic::LocalApic;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Lapic {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub fn init() -> Lapic {
    // SAFETY: We are initializing the LAPIC, so reading the base address is necessary and safe
    // as we trust the hardware/bootloader configuration at this stage.
    let xapic_base = unsafe { xapic_base() };
    let phys_addr = PhysAddr::new(xapic_base);
    let frame = PhysFrame::containing_address(phys_addr);

    let segment = VirtualMemoryHigherHalf
        .reserve(1)
        .expect("should have enough virtual memory for LAPIC");
    let virt_page = Page::containing_address(segment.start);

    let address_space = AddressSpace::kernel();

    // Unmap first in case bootloader left something mapped here
    // (ignore errors if nothing was mapped)
    let _ = address_space.unmap(virt_page);

    // Now map our LAPIC region
    address_space
        .map::<Size4KiB>(
            virt_page,
            frame,
            PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_CACHE
                | PageTableFlags::NO_EXECUTE,
        )
        .expect("should be able to map LAPIC region after unmapping");

    let mut lapic = LocalApicBuilder::new()
        .timer_vector(InterruptIndex::Timer.as_usize())
        .error_vector(InterruptIndex::LapicErr.as_usize())
        .spurious_vector(InterruptIndex::Spurious.as_usize())
        .set_xapic_base(segment.start.as_u64())
        .timer_mode(TimerMode::Periodic)
        .timer_initial(312_500)
        .timer_divide(TimerDivide::Div16)
        .build()
        .expect("should be able to build lapic");

    // SAFETY: Enabling the LAPIC is safe as we have configured it correctly above.
    unsafe {
        lapic.enable();
    }

    Lapic {
        _segment: segment,
        inner: lapic,
    }
}
