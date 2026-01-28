//! ARM64 Memory Management Initialization
//!
//! Sets up kernel page tables and enables the MMU.

use core::ptr;

use super::dtb;
use super::mem::{self, PAGE_SIZE, pte_flags};
use super::paging::{self, PageTable, PageTableWalker};
use super::phys::{self, PhysFrame};

/// Kernel L0 page table (statically allocated for bootstrap)
#[repr(C, align(4096))]
struct BootPageTables {
    l0: PageTable,
    l1_low: PageTable,  // For identity mapping (low addresses)
    l1_high: PageTable, // For kernel mapping (high addresses)
}

static mut BOOT_TABLES: BootPageTables = BootPageTables {
    l0: PageTable::empty(),
    l1_low: PageTable::empty(),
    l1_high: PageTable::empty(),
};

/// Initialize ARM64 memory management
///
/// This function:
/// 1. Initializes the physical memory allocator (stage 1)
/// 2. Sets up kernel page tables with identity + higher-half mappings
/// 3. Configures MAIR and TCR
/// 4. Enables the MMU with new page tables
pub fn init() {
    log::info!("Initializing ARM64 memory management...");

    // Initialize physical memory allocator (stage 1 - bump allocator)
    phys::init_stage1();

    // Get memory info from DTB
    let dtb_info = dtb::info();
    let total_memory = dtb_info.total_memory;

    log::info!("Setting up kernel page tables...");

    // Set up initial page tables
    unsafe {
        setup_kernel_page_tables(total_memory);
    }

    log::info!("ARM64 memory management initialized");
}

/// Set up kernel page tables
///
/// Creates identity mapping for low memory and higher-half mapping for kernel.
unsafe fn setup_kernel_page_tables(total_memory: usize) {
    let boot_tables = unsafe { &mut *(&raw mut BOOT_TABLES) };

    // Clear all tables
    boot_tables.l0.zero();
    boot_tables.l1_low.zero();
    boot_tables.l1_high.zero();

    // Get physical addresses of tables
    let l0_phys = &raw const boot_tables.l0 as usize;
    let l1_low_phys = &raw const boot_tables.l1_low as usize;
    let l1_high_phys = &raw const boot_tables.l1_high as usize;

    // L0[0] -> L1_low (identity mapping for first 512GB)
    *boot_tables.l0.entry_mut(0) = paging::PageTableEntry::table(l1_low_phys);

    // L0[511] -> L1_high (kernel higher-half mapping, 0xFFFF_8000_0000_0000+)
    // Index 511 covers 0xFFFF_8000_0000_0000 - 0xFFFF_FFFF_FFFF_FFFF
    *boot_tables.l0.entry_mut(256) = paging::PageTableEntry::table(l1_high_phys);

    // Set up identity mapping using 1GB blocks for simplicity
    // Map first N GB where N depends on total memory
    let gb_to_map = ((total_memory + (1 << 30) - 1) >> 30).max(4); // At least 4GB

    for i in 0..gb_to_map.min(512) {
        let phys_addr = i << 30; // 1GB per entry

        // L1 block descriptor for 1GB mapping
        let block_flags = pte_flags::VALID
            | pte_flags::AF
            | pte_flags::SH_INNER
            | pte_flags::attr_index(mem::mair::NORMAL_WB);

        *boot_tables.l1_low.entry_mut(i) = paging::PageTableEntry::block(phys_addr, block_flags);

        // Also map in higher-half (same physical memory)
        if i < 512 {
            *boot_tables.l1_high.entry_mut(i) =
                paging::PageTableEntry::block(phys_addr, block_flags);
        }
    }

    // Configure MAIR (memory attributes)
    unsafe {
        paging::configure_mair();
    }

    // Configure TCR (translation control)
    unsafe {
        paging::configure_tcr();
    }

    // Set TTBR0 (user/identity mapping) and TTBR1 (kernel mapping)
    unsafe {
        paging::set_ttbr0(l0_phys);
        paging::set_ttbr1(l0_phys);
    }

    log::info!(
        "Page tables configured: L0={:#x}, mapped {}GB",
        l0_phys,
        gb_to_map
    );
}

/// Get the kernel page table root physical address
pub fn kernel_page_table_phys() -> usize {
    unsafe { &raw const BOOT_TABLES.l0 as usize }
}

/// Create a new user address space
///
/// Allocates a new L0 table and copies kernel mappings into it.
pub fn create_user_address_space() -> Option<*mut PageTable> {
    // Allocate a new L0 table
    let frame = phys::allocate_frame()?;
    let l0_ptr = frame.addr() as *mut PageTable;

    unsafe {
        // Zero the table
        ptr::write_bytes(l0_ptr, 0, 1);

        // Copy kernel mappings (upper half - entry 256-511)
        let boot_l0 = &*(&raw const BOOT_TABLES.l0);
        let new_l0 = &mut *l0_ptr;

        for i in 256..512 {
            *new_l0.entry_mut(i) = *boot_l0.entry(i);
        }
    }

    Some(l0_ptr)
}

/// Initialize stage 2 of memory management (after heap is available)
pub fn init_stage2() {
    phys::init_stage2();
    log::info!("ARM64 memory management stage 2 initialized");
}
