/// Initialize paging (4KB granule, 48-bit VA)
pub fn init() {
    // TODO: Setup kernel page tables
    // For now, we assume bootloader has set up identity mapping
    log::info!("ARM paging initialized (4KB granule, 48-bit VA)");
}

/// Flush TLB
pub fn flush_tlb() {
    unsafe {
        core::arch::asm!("dsb ishst", "tlbi vmalle1is", "dsb ish", "isb");
    }
}

/// Flush TLB for specific virtual address
pub fn flush_tlb_page(vaddr: usize) {
    unsafe {
        core::arch::asm!(
            "dsb ishst",
            "tlbi vae1is, {}",
            "dsb ish",
            "isb",
            in(reg) vaddr >> 12
        );
    }
}

/// Set page table base (TTBR0_EL1 for user, TTBR1_EL1 for kernel)
pub unsafe fn set_user_page_table(base: u64) {
    core::arch::asm!("msr ttbr0_el1, {}", in(reg) base);
    flush_tlb();
}

pub unsafe fn set_kernel_page_table(base: u64) {
    core::arch::asm!("msr ttbr1_el1, {}", in(reg) base);
    flush_tlb();
}

/// Get current page table bases
pub fn get_user_page_table() -> u64 {
    let ttbr0: u64;
    unsafe {
        core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr0);
    }
    ttbr0
}

pub fn get_kernel_page_table() -> u64 {
    let ttbr1: u64;
    unsafe {
        core::arch::asm!("mrs {}, ttbr1_el1", out(reg) ttbr1);
    }
    ttbr1
}

/// Page table entry for 4KB granule
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn new() -> Self {
        Self(0)
    }

    pub fn is_valid(&self) -> bool {
        self.0 & 0x1 != 0
    }

    pub fn is_table(&self) -> bool {
        (self.0 & 0x3) == 0x3
    }

    pub fn is_page(&self) -> bool {
        (self.0 & 0x3) == 0x3 && (self.0 & (1 << 1)) != 0
    }

    pub fn is_block(&self) -> bool {
        (self.0 & 0x3) == 0x1
    }

    pub fn address(&self) -> u64 {
        self.0 & 0x0000_FFFF_FFFF_F000
    }

    pub fn set_address(&mut self, addr: u64) {
        self.0 = (self.0 & !0x0000_FFFF_FFFF_F000) | (addr & 0x0000_FFFF_FFFF_F000);
    }

    pub fn set_valid(&mut self, valid: bool) {
        if valid {
            self.0 |= 0x1;
        } else {
            self.0 &= !0x1;
        }
    }

    pub fn set_table(&mut self) {
        self.0 = (self.0 & !0x3) | 0x3;
    }

    pub fn set_page(&mut self) {
        self.0 = (self.0 & !0x3) | 0x3;
    }

    pub fn set_block(&mut self) {
        self.0 = (self.0 & !0x3) | 0x1;
    }

    // Access permissions
    pub fn set_user_accessible(&mut self, accessible: bool) {
        if accessible {
            self.0 |= 1 << 6; // AP[1] = 1 for user access
        } else {
            self.0 &= !(1 << 6);
        }
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        if read_only {
            self.0 |= 1 << 7; // AP[2] = 1 for read-only
        } else {
            self.0 &= !(1 << 7);
        }
    }

    pub fn set_executable(&mut self, executable: bool) {
        if !executable {
            self.0 |= 1 << 54; // UXN = 1 for non-executable
        } else {
            self.0 &= !(1 << 54);
        }
    }
}

/// Page table (512 entries for 4KB granule)
#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); 512],
        }
    }

    pub fn entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    pub fn entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }
}
