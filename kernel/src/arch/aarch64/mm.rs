//! ARM64 Memory Management Initialization
//!
//! Sets up kernel page tables and enables the MMU.

use core::ptr;

use super::dtb;
use super::mem::{self, pte_flags};
use super::paging::{self, PageTable};
use super::phys::{self};

/// Kernel L0 page tables (statically allocated for bootstrap)
#[repr(C, align(4096))]
struct BootPageTables {
    l0_user: PageTable,   // For identity mapping (TTBR0)
    l0_kernel: PageTable, // For kernel mapping (TTBR1)
    l1_low: PageTable,    // For identity mapping (low addresses)
    l1_high: PageTable,   // For kernel mapping (high addresses)
}

static mut BOOT_TABLES: BootPageTables = BootPageTables {
    l0_user: PageTable::empty(),
    l0_kernel: PageTable::empty(),
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
/// 5. Maps and initializes the kernel heap
pub fn init() {
    log::info!("Initializing ARM64 memory management...");

    // Initialize physical memory allocator (stage 1 - bump allocator)
    phys::init_stage1();

    // Get memory info from DTB
    let dtb_info = dtb::info();
    let total_memory = dtb_info.total_memory;

    log::info!("Setting up kernel page tables...");

    // Set up initial page tables
    // SAFETY: We are in early boot, single-threaded, and have exclusive access to memory.
    // The total_memory value comes from the DTB which was validated earlier.
    unsafe {
        setup_kernel_page_tables(total_memory);

        // Configure MAIR (memory attributes)
        paging::configure_mair();

        // Configure TCR (translation control)
        paging::configure_tcr();

        // Set TTBR0 (user/identity mapping) and TTBR1 (kernel mapping)
        let l0_user_phys = &raw const BOOT_TABLES.l0_user as usize;
        let l0_kernel_phys = &raw const BOOT_TABLES.l0_kernel as usize;
        paging::set_ttbr0(l0_user_phys);
        paging::set_ttbr1(l0_kernel_phys);

        // Enable the MMU
        paging::enable_mmu();
    }

    log::info!("ARM64 memory management initialized");
}

/// Set up kernel page tables
///
/// Creates identity mapping for low memory and higher-half mapping for kernel.
///
/// # Safety
///
/// This function must be called only during early boot. It accesses the static
/// `BOOT_TABLES` which is mutable and not thread-safe. It assumes `total_memory`
/// correctly reflects the physical memory size available.
unsafe fn setup_kernel_page_tables(total_memory: usize) {
    #[allow(clippy::deref_addrof)]
    // SAFETY: We are in early boot (single core) and this is the only access to BOOT_TABLES.
    let boot_tables = unsafe { &mut *(&raw mut BOOT_TABLES) };

    // Clear all tables
    boot_tables.l0_user.zero();
    boot_tables.l0_kernel.zero();
    boot_tables.l1_low.zero();
    boot_tables.l1_high.zero();

    // Get physical addresses of tables
    let l1_low_phys = &raw const boot_tables.l1_low as usize;
    let l1_high_phys = &raw const boot_tables.l1_high as usize;

    // L0_user[0] -> L1_low (identity mapping for first 512GB)
    *boot_tables.l0_user.entry_mut(0) = paging::PageTableEntry::table(l1_low_phys);

    // L0_kernel[0] -> L1_low (also identity map in kernel table for convenience)
    // This allows the PageTableWalker to translate identity-mapped addresses
    // using the kernel page table root.
    *boot_tables.l0_kernel.entry_mut(0) = paging::PageTableEntry::table(l1_low_phys);

    // L0_kernel[256] -> L1_high (kernel higher-half mapping, 0xFFFF_8000_0000_0000+)
    // Index 256 covers 0xFFFF_8000_0000_0000 - 0xFFFF_807F_FFFF_FFFF
    *boot_tables.l0_kernel.entry_mut(256) = paging::PageTableEntry::table(l1_high_phys);

    // Set up identity mapping using 1GB blocks for simplicity
    // Map first N GB where N depends on total memory
    let gb_to_map = ((total_memory + (1 << 30) - 1) >> 30).max(4); // At least 4GB

    for i in 0..gb_to_map.min(512) {
        let phys_addr = i << 30; // 1GB per entry

        // L1 block descriptor for 1GB mapping
        let mut block_flags = pte_flags::VALID
            | pte_flags::AF
            | pte_flags::SH_INNER;

        // QEMU virt memory map:
        // 0x0000_0000 - 0x3FFF_FFFF: Devices (Flash, GIC, UART, etc.)
        // 0x4000_0000 - ...        : RAM
        //
        // Map the first 1GB as Device-nGnRE.
        // Map the rest as Normal WB.
        if i == 0 {
            block_flags |= pte_flags::attr_index(mem::mair::DEVICE_NGNRE);
            block_flags |= pte_flags::UXN | pte_flags::PXN;
        } else {
            block_flags |= pte_flags::attr_index(mem::mair::NORMAL_WB);
        }

        *boot_tables.l1_low.entry_mut(i) = paging::PageTableEntry::block(phys_addr, block_flags);

        // Also map in higher-half (same physical memory)
        if i < 512 {
            *boot_tables.l1_high.entry_mut(i) =
                paging::PageTableEntry::block(phys_addr, block_flags);
        }
    }

    log::info!(
        "Bootstrap page tables configured, mapped {}GB",
        gb_to_map
    );
}

/// Get the kernel page table root physical address
pub fn kernel_page_table_phys() -> usize {
    // SAFETY: Accessing the address of the static BOOT_TABLES.l0_kernel.
    unsafe { &raw const BOOT_TABLES.l0_kernel as usize }
}

/// Create a new user address space
///
/// Allocates a new L0 table and copies kernel mappings into it.
pub fn create_user_address_space() -> Option<*mut PageTable> {
    // Allocate a new L0 table
    let frame = phys::allocate_frame::<crate::arch::types::Size4KiB>()?;
    let l0_ptr = frame.addr() as *mut PageTable;

    // SAFETY: We allocated a fresh frame, so writing to it is safe.
    unsafe {
        // Zero the table
        ptr::write_bytes(l0_ptr, 0, 1);

        // Copy kernel mappings (upper half - entry 256-511)
        #[allow(clippy::deref_addrof)]
        let boot_l0_kernel = &*(&raw const BOOT_TABLES.l0_kernel);
        let new_l0 = &mut *l0_ptr;

        for i in 256..512 {
            *new_l0.entry_mut(i) = *boot_l0_kernel.entry(i);
        }

        // Copy identity mapping (entry 0) from l0_user so kernel physical addresses work
        // This is required because the kernel is linked at physical addresses (0x40080000)
        // and functions like TaskCleanup::run will be resolved to these low addresses.
        #[allow(clippy::deref_addrof)]
        let boot_l0_user = &*(&raw const BOOT_TABLES.l0_user);
        *new_l0.entry_mut(0) = *boot_l0_user.entry(0);
    }

    Some(l0_ptr)
}

/// Initialize stage 2 of memory management (after heap is available)
pub fn init_stage2() {
    phys::init_stage2();
    log::info!("ARM64 memory management stage 2 initialized");
}
