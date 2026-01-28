//! ARM64 Paging Implementation
//!
//! Implements 4-level page tables for ARM64 with 4KB granule and 48-bit VA.
//! Supports both kernel (TTBR1) and user (TTBR0) address spaces.

use core::ptr;

use super::mem::{
    ENTRIES_PER_TABLE, L0_SHIFT, L1_SHIFT, L2_SHIFT, L3_SHIFT, PAGE_SIZE, mair, pte_flags,
};
use super::phys::{self, PhysFrame};

/// Page table entry for 4KB granule
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    /// Create an empty (invalid) entry
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Create a table descriptor pointing to next level page table
    pub const fn table(phys_addr: usize) -> Self {
        Self((phys_addr as u64) | pte_flags::VALID | pte_flags::TABLE)
    }

    /// Create a page descriptor (L3 entry)
    pub const fn page(phys_addr: usize, flags: u64) -> Self {
        Self((phys_addr as u64) | flags)
    }

    /// Create a block descriptor (L1/L2 entry for large mappings)
    pub const fn block(phys_addr: usize, flags: u64) -> Self {
        // Block descriptor has bit[1] = 0, bit[0] = 1
        Self((phys_addr as u64) | (flags & !pte_flags::TABLE) | pte_flags::VALID)
    }

    /// Check if entry is valid
    pub const fn is_valid(&self) -> bool {
        self.0 & pte_flags::VALID != 0
    }

    /// Check if entry is a table descriptor (points to next level)
    pub const fn is_table(&self) -> bool {
        (self.0 & 0x3) == 0x3
    }

    /// Check if entry is a block descriptor
    pub const fn is_block(&self) -> bool {
        (self.0 & 0x3) == 0x1
    }

    /// Get the physical address from this entry
    pub const fn addr(&self) -> usize {
        (self.0 & 0x0000_FFFF_FFFF_F000) as usize
    }

    /// Get raw value
    pub const fn raw(&self) -> u64 {
        self.0
    }

    /// Set the entry value
    pub fn set(&mut self, value: u64) {
        self.0 = value;
    }

    /// Clear the entry
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

/// Page table (512 entries for 4KB granule)
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; ENTRIES_PER_TABLE],
}

impl PageTable {
    /// Create an empty page table
    pub const fn empty() -> Self {
        Self {
            entries: [PageTableEntry::empty(); ENTRIES_PER_TABLE],
        }
    }

    /// Get entry at index
    pub fn entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    /// Get mutable entry at index
    pub fn entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }

    /// Zero all entries
    pub fn zero(&mut self) {
        for entry in &mut self.entries {
            entry.clear();
        }
    }
}

/// Extract page table indices from virtual address
pub const fn va_to_indices(va: usize) -> [usize; 4] {
    [
        (va >> L0_SHIFT) & 0x1FF,
        (va >> L1_SHIFT) & 0x1FF,
        (va >> L2_SHIFT) & 0x1FF,
        (va >> L3_SHIFT) & 0x1FF,
    ]
}

/// Page table walker for creating and walking page tables
pub struct PageTableWalker {
    root: *mut PageTable,
}

impl PageTableWalker {
    /// Create a new walker with the given root table
    ///
    /// # Safety
    /// The root pointer must be valid and properly aligned.
    pub unsafe fn new(root: *mut PageTable) -> Self {
        Self { root }
    }

    /// Get the root table physical address
    pub fn root_phys(&self) -> usize {
        self.root as usize
    }

    /// Map a single 4KB page
    ///
    /// Creates intermediate tables as needed.
    pub fn map_page(&mut self, virt: usize, phys: usize, flags: u64) -> Result<(), &'static str> {
        let indices = va_to_indices(virt);

        // Walk/create L0 -> L1 -> L2 -> L3 using raw pointers to avoid borrow issues
        let l1_ptr = Self::get_or_create_table_ptr(self.root, indices[0])?;
        let l2_ptr = Self::get_or_create_table_ptr(l1_ptr, indices[1])?;
        let l3_ptr = Self::get_or_create_table_ptr(l2_ptr, indices[2])?;

        // Set L3 entry (page descriptor)
        let l3 = unsafe { &mut *l3_ptr };
        let entry = l3.entry_mut(indices[3]);
        if entry.is_valid() {
            return Err("Page already mapped");
        }

        *entry = PageTableEntry::page(phys, flags);
        Ok(())
    }

    /// Map a range of pages
    pub fn map_range(
        &mut self,
        virt_start: usize,
        phys_start: usize,
        size: usize,
        flags: u64,
    ) -> Result<(), &'static str> {
        let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        for i in 0..pages {
            let virt = virt_start + i * PAGE_SIZE;
            let phys = phys_start + i * PAGE_SIZE;
            self.map_page(virt, phys, flags)?;
        }

        Ok(())
    }

    /// Unmap a page and return its physical address
    pub fn unmap_page(&mut self, virt: usize) -> Result<usize, &'static str> {
        let indices = va_to_indices(virt);

        let l0 = unsafe { &mut *self.root };
        let l1 = self
            .get_table(l0, indices[0])
            .ok_or("L1 table not present")?;
        let l2 = self
            .get_table(l1, indices[1])
            .ok_or("L2 table not present")?;
        let l3 = self
            .get_table(l2, indices[2])
            .ok_or("L3 table not present")?;

        let entry = l3.entry_mut(indices[3]);
        if !entry.is_valid() {
            return Err("Page not mapped");
        }

        let phys = entry.addr();
        entry.clear();

        // Invalidate TLB for this address
        flush_tlb_page(virt);

        Ok(phys)
    }

    /// Translate virtual address to physical address
    pub fn translate(&self, virt: usize) -> Option<usize> {
        let indices = va_to_indices(virt);
        let offset = virt & (PAGE_SIZE - 1);

        let l0 = unsafe { &*self.root };
        let l1 = self.get_table_readonly(l0, indices[0])?;
        let l2 = self.get_table_readonly(l1, indices[1])?;
        let l3 = self.get_table_readonly(l2, indices[2])?;

        let entry = l3.entry(indices[3]);
        if entry.is_valid() {
            Some(entry.addr() + offset)
        } else {
            None
        }
    }

    /// Get or create a table at the given index (using raw pointers)
    fn get_or_create_table_ptr(
        table: *mut PageTable,
        index: usize,
    ) -> Result<*mut PageTable, &'static str> {
        let entry = unsafe { (*table).entry_mut(index) };

        if entry.is_valid() {
            if !entry.is_table() {
                return Err("Entry is block, not table");
            }
            // Table already exists
            Ok(entry.addr() as *mut PageTable)
        } else {
            // Allocate new table
            let frame = phys::allocate_frame().ok_or("Out of memory for page table")?;
            let table_ptr = frame.addr() as *mut PageTable;

            // Zero the new table
            unsafe {
                ptr::write_bytes(table_ptr, 0, 1);
            }

            // Set table descriptor
            *entry = PageTableEntry::table(frame.addr());

            Ok(table_ptr)
        }
    }

    /// Get existing table at index (mutable)
    fn get_table<'a>(&self, table: &'a mut PageTable, index: usize) -> Option<&'a mut PageTable> {
        let entry = table.entry(index);
        if entry.is_valid() && entry.is_table() {
            let next_table = entry.addr() as *mut PageTable;
            Some(unsafe { &mut *next_table })
        } else {
            None
        }
    }

    /// Get existing table at index (readonly)
    fn get_table_readonly(&self, table: &PageTable, index: usize) -> Option<&PageTable> {
        let entry = table.entry(index);
        if entry.is_valid() && entry.is_table() {
            let next_table = entry.addr() as *const PageTable;
            Some(unsafe { &*next_table })
        } else {
            None
        }
    }
}

/// Flush entire TLB
pub fn flush_tlb() {
    unsafe {
        core::arch::asm!(
            "dsb ishst",
            "tlbi vmalle1is",
            "dsb ish",
            "isb",
            options(nostack, preserves_flags)
        );
    }
}

/// Flush TLB for specific virtual address
pub fn flush_tlb_page(vaddr: usize) {
    unsafe {
        core::arch::asm!(
            "dsb ishst",
            "tlbi vae1is, {0}",
            "dsb ish",
            "isb",
            in(reg) vaddr >> 12,
            options(nostack, preserves_flags)
        );
    }
}

/// Set TTBR0_EL1 (user page table base)
///
/// # Safety
/// The base address must point to a valid page table.
pub unsafe fn set_ttbr0(base: usize) {
    unsafe {
        core::arch::asm!(
            "msr ttbr0_el1, {0}",
            "isb",
            in(reg) base,
            options(nostack, preserves_flags)
        );
    }
    flush_tlb();
}

/// Set TTBR1_EL1 (kernel page table base)
///
/// # Safety
/// The base address must point to a valid page table.
pub unsafe fn set_ttbr1(base: usize) {
    unsafe {
        core::arch::asm!(
            "msr ttbr1_el1, {0}",
            "isb",
            in(reg) base,
            options(nostack, preserves_flags)
        );
    }
    flush_tlb();
}

/// Get current TTBR0_EL1 value
pub fn get_ttbr0() -> usize {
    let value: usize;
    unsafe {
        core::arch::asm!(
            "mrs {0}, ttbr0_el1",
            out(reg) value,
            options(nostack, preserves_flags)
        );
    }
    value
}

/// Get current TTBR1_EL1 value
pub fn get_ttbr1() -> usize {
    let value: usize;
    unsafe {
        core::arch::asm!(
            "mrs {0}, ttbr1_el1",
            out(reg) value,
            options(nostack, preserves_flags)
        );
    }
    value
}

/// Configure MAIR_EL1 with standard memory attributes
///
/// # Safety
/// Must be called before enabling the MMU with new page tables.
pub unsafe fn configure_mair() {
    unsafe {
        core::arch::asm!(
            "msr mair_el1, {0}",
            "isb",
            in(reg) mair::MAIR_VALUE,
            options(nostack, preserves_flags)
        );
    }
}

/// Configure TCR_EL1 for 48-bit VA, 4KB granule
///
/// # Safety
/// Must be called before enabling the MMU with new page tables.
pub unsafe fn configure_tcr() {
    // TCR_EL1 configuration:
    // - T0SZ = 16 (48-bit VA for TTBR0)
    // - T1SZ = 16 (48-bit VA for TTBR1)
    // - TG0 = 0 (4KB granule for TTBR0)
    // - TG1 = 2 (4KB granule for TTBR1)
    // - SH0/SH1 = 3 (Inner shareable)
    // - ORGN0/ORGN1 = 1 (Write-back cacheable)
    // - IRGN0/IRGN1 = 1 (Write-back cacheable)
    // - IPS = based on physical address size

    let tcr: u64 = (16 << 0)        // T0SZ
                 | (16 << 16)       // T1SZ
                 | (0b00 << 14)     // TG0 = 4KB
                 | (0b10 << 30)     // TG1 = 4KB
                 | (0b11 << 12)     // SH0 = Inner Shareable
                 | (0b11 << 28)     // SH1 = Inner Shareable
                 | (0b01 << 10)     // ORGN0 = Write-back
                 | (0b01 << 26)     // ORGN1 = Write-back
                 | (0b01 << 8)      // IRGN0 = Write-back
                 | (0b01 << 24)     // IRGN1 = Write-back
                 | (0b101 << 32); // IPS = 48-bit PA (256TB)

    unsafe {
        core::arch::asm!(
            "msr tcr_el1, {0}",
            "isb",
            in(reg) tcr,
            options(nostack, preserves_flags)
        );
    }
}

/// Initialize paging subsystem
pub fn init() {
    log::info!("ARM64 paging initialized (4KB granule, 48-bit VA)");
}
