use core::ops::{Deref, DerefMut};

use acpi::{InterruptModel, PlatformInfo};
use conquer_once::spin::OnceCell;
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

use crate::acpi::acpi_tables;
use crate::mem::address_space::AddressSpace;
use crate::mem::virt::{OwnedSegment, VirtualMemoryAllocator, VirtualMemoryHigherHalf};

static IO_APIC: OnceCell<Mutex<IoApic>> = OnceCell::uninit();

pub fn io_apic() -> &'static Mutex<IoApic> {
    IO_APIC.get().expect("IOAPIC not initialized")
}

pub struct IoApic {
    _segment: OwnedSegment<'static>,
    inner: x2apic::ioapic::IoApic,
}

impl Deref for IoApic {
    type Target = x2apic::ioapic::IoApic;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for IoApic {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub fn init() {
    disable_8259();

    let acpi_tables = acpi_tables();
    let acpi_tables = acpi_tables.lock();

    let Ok(platform_info) = PlatformInfo::new(&acpi_tables) else {
        panic!("failed to get platform info");
    };
    let InterruptModel::Apic(apic) = platform_info.interrupt_model else {
        panic!("Unsupported interrupt model");
    };

    let apics = apic.io_apics;
    assert_eq!(
        apics.len(),
        1,
        "only exactly one IOAPIC is supported for now"
    );

    let apic = apics.last().unwrap();
    let phys_addr = PhysAddr::new(u64::from(apic.address));

    let segment = VirtualMemoryHigherHalf
        .reserve(1)
        .expect("should have enough virtual memory for IOAPIC");
    AddressSpace::kernel()
        .map::<Size4KiB>(
            Page::containing_address(segment.start),
            PhysFrame::containing_address(phys_addr),
            PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_CACHE
                | PageTableFlags::NO_EXECUTE,
        )
        .unwrap();
    let start_addr = segment.start;

    let ioapic = IoApic {
        _segment: segment,
        // SAFETY: We have just mapped the IOAPIC base address to `start_addr`.
        // The `ioapic` crate requires an unsafe call to initialize from a raw address.
        inner: unsafe { x2apic::ioapic::IoApic::new(start_addr.as_u64()) },
    };
    IO_APIC.init_once(|| Mutex::new(ioapic));
}

#[allow(clippy::similar_names)]
fn disable_8259() {
    // SAFETY: We are writing to standard legacy PIC IO ports (0x20, 0x21, 0xA0, 0xA1)
    // to disable the 8259 PIC. This is a standard initialization step on x86_64
    // to switch to APIC. We also use port 0x80 for IO delays.
    unsafe {
        let mut cmd_8259a = Port::<u8>::new(0x20);
        let mut data_8259a = Port::<u8>::new(0x21);
        let mut cmd_8259b = Port::<u8>::new(0xa0);
        let mut data_8259b = Port::<u8>::new(0xa1);

        let mut spin_port = Port::<u8>::new(0x80);
        let mut spin = || spin_port.write(0);

        cmd_8259a.write(0x11);
        cmd_8259b.write(0x11);
        spin();

        data_8259a.write(0xf8);
        data_8259b.write(0xff);
        spin();

        data_8259a.write(0b100);
        spin();

        data_8259b.write(0b10);
        spin();

        data_8259a.write(0x1);
        data_8259b.write(0x1);
        spin();

        data_8259a.write(u8::MAX);
        data_8259b.write(u8::MAX);
    };
}
