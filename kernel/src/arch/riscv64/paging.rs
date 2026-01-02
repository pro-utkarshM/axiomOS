use riscv::register::satp;

/// Initialize paging (Sv39)
pub fn init() {
    // TODO: Setup kernel page tables
    // For now, we assume bootloader has set up identity mapping
    log::info!("RISC-V paging initialized (Sv39)");
}

/// Flush TLB
pub fn flush_tlb() {
    unsafe {
        riscv::asm::sfence_vma_all();
    }
}

/// Flush TLB for specific virtual address
pub fn flush_tlb_page(vaddr: usize) {
    unsafe {
        riscv::asm::sfence_vma(vaddr, 0);
    }
}

/// Set page table base (satp register)
pub unsafe fn set_page_table(ppn: usize) {
    // Sv39 mode (mode = 8)
    let satp_value = (8 << 60) | (ppn & 0xFFFFFFFFFFF);
    satp::write(satp_value);
    flush_tlb();
}

/// Get current page table base
pub fn get_page_table() -> usize {
    let satp_value = satp::read();
    satp_value & 0xFFFFFFFFFFF
}

/// Page table entry for Sv39
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

    pub fn is_readable(&self) -> bool {
        self.0 & 0x2 != 0
    }

    pub fn is_writable(&self) -> bool {
        self.0 & 0x4 != 0
    }

    pub fn is_executable(&self) -> bool {
        self.0 & 0x8 != 0
    }

    pub fn is_user(&self) -> bool {
        self.0 & 0x10 != 0
    }

    pub fn ppn(&self) -> usize {
        ((self.0 >> 10) & 0xFFFFFFFFFFF) as usize
    }

    pub fn set_ppn(&mut self, ppn: usize) {
        self.0 = (self.0 & !0xFFFFFFFFFC00) | ((ppn as u64) << 10);
    }

    pub fn set_valid(&mut self, valid: bool) {
        if valid {
            self.0 |= 0x1;
        } else {
            self.0 &= !0x1;
        }
    }

    pub fn set_readable(&mut self, readable: bool) {
        if readable {
            self.0 |= 0x2;
        } else {
            self.0 &= !0x2;
        }
    }

    pub fn set_writable(&mut self, writable: bool) {
        if writable {
            self.0 |= 0x4;
        } else {
            self.0 &= !0x4;
        }
    }

    pub fn set_executable(&mut self, executable: bool) {
        if executable {
            self.0 |= 0x8;
        } else {
            self.0 &= !0x8;
        }
    }

    pub fn set_user(&mut self, user: bool) {
        if user {
            self.0 |= 0x10;
        } else {
            self.0 &= !0x10;
        }
    }
}

/// Sv39 page table (512 entries)
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
