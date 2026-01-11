//! ARM64 Memory Management Constants and Layout
//!
//! Defines the virtual address space layout for the kernel on ARM64.
//! Uses 4KB granule with 48-bit virtual addresses.

/// Page size (4KB granule)
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;
pub const PAGE_MASK: usize = !(PAGE_SIZE - 1);

/// Page table constants for 4KB granule, 48-bit VA
pub const ENTRIES_PER_TABLE: usize = 512;
pub const TABLE_SHIFT: usize = 9; // log2(512)

/// Virtual address bit width
pub const VA_BITS: usize = 48;

/// Page table level shifts (for 4KB granule)
pub const L0_SHIFT: usize = 39; // 512GB per entry
pub const L1_SHIFT: usize = 30; // 1GB per entry
pub const L2_SHIFT: usize = 21; // 2MB per entry
pub const L3_SHIFT: usize = 12; // 4KB per entry (page)

/// Block sizes at each level
pub const L1_BLOCK_SIZE: usize = 1 << L1_SHIFT; // 1GB
pub const L2_BLOCK_SIZE: usize = 1 << L2_SHIFT; // 2MB

/// Kernel virtual address space layout (upper half: 0xFFFF_0000_0000_0000+)
pub mod kernel {
    /// Start of kernel address space (upper half)
    pub const BASE: usize = 0xFFFF_0000_0000_0000;

    /// Physical memory direct map region
    /// Maps all physical memory starting here
    pub const PHYS_MAP_BASE: usize = 0xFFFF_8000_0000_0000;
    pub const PHYS_MAP_SIZE: usize = 0x0000_4000_0000_0000; // 64TB max

    /// Kernel image base (where kernel is loaded)
    /// Identity mapped from physical 0x80000
    pub const IMAGE_BASE: usize = 0xFFFF_FFFF_8008_0000;

    /// Kernel heap region
    pub const HEAP_BASE: usize = 0xFFFF_C000_0000_0000;
    pub const HEAP_SIZE: usize = 0x0000_0001_0000_0000; // 4GB max heap

    /// Kernel stack region (per-CPU stacks)
    pub const STACK_BASE: usize = 0xFFFF_D000_0000_0000;
    pub const STACK_SIZE: usize = 64 * 1024; // 64KB per stack

    /// MMIO region for device mappings
    pub const MMIO_BASE: usize = 0xFFFF_E000_0000_0000;
    pub const MMIO_SIZE: usize = 0x0000_1000_0000_0000; // 16TB for MMIO
}

/// User virtual address space layout (lower half: 0x0000_0000_0000_0000 - 0x0000_FFFF_FFFF_FFFF)
pub mod user {
    /// Start of user address space
    pub const BASE: usize = 0x0000_0000_0000_0000;

    /// End of user address space (exclusive)
    pub const END: usize = 0x0001_0000_0000_0000; // 256TB

    /// User stack grows down from here
    pub const STACK_TOP: usize = 0x0000_8000_0000_0000;

    /// User heap starts here
    pub const HEAP_BASE: usize = 0x0000_0000_1000_0000;
}

/// Raspberry Pi 5 specific physical addresses
#[cfg(feature = "rpi5")]
pub mod rpi5 {
    /// DRAM starts at 0
    pub const DRAM_BASE: usize = 0x0;

    /// Kernel is loaded at 0x80000 by the firmware
    pub const KERNEL_PHYS_BASE: usize = 0x8_0000;

    /// RP1 (southbridge) base through PCIe
    pub const RP1_BASE: usize = 0x1F00_0000_0000;

    /// GIC (interrupt controller) on BCM2712
    pub const GIC_DIST_BASE: usize = 0xFF84_1000;
    pub const GIC_CPU_BASE: usize = 0xFF84_2000;
}

/// Memory attribute indices for MAIR_EL1
pub mod mair {
    /// Device-nGnRnE memory (strongly ordered)
    pub const DEVICE_NGNRNE: u8 = 0;
    /// Device-nGnRE memory
    pub const DEVICE_NGNRE: u8 = 1;
    /// Normal memory, outer/inner write-back cacheable
    pub const NORMAL_WB: u8 = 2;
    /// Normal memory, outer/inner non-cacheable
    pub const NORMAL_NC: u8 = 3;

    /// MAIR_EL1 value encoding all memory types
    pub const MAIR_VALUE: u64 = 0x00_44_FF_00;
    // Index 0: Device-nGnRnE (0x00)
    // Index 1: Device-nGnRE (0x04) - not used, placeholder
    // Index 2: Normal WB (0xFF = outer WB, inner WB)
    // Index 3: Normal NC (0x44 = outer NC, inner NC)
}

/// Page table entry flags for ARM64
pub mod pte_flags {
    /// Valid bit
    pub const VALID: u64 = 1 << 0;
    /// Table descriptor (for L0-L2)
    pub const TABLE: u64 = 1 << 1;
    /// Page descriptor (for L3)
    pub const PAGE: u64 = 1 << 1;
    /// Access flag (must be set for hardware access)
    pub const AF: u64 = 1 << 10;
    /// Shareability: Inner shareable
    pub const SH_INNER: u64 = 3 << 8;
    /// Shareability: Outer shareable
    pub const SH_OUTER: u64 = 2 << 8;

    /// Access permissions
    pub const AP_RW_EL1: u64 = 0 << 6; // RW at EL1, no access at EL0
    pub const AP_RW_ALL: u64 = 1 << 6; // RW at EL1 and EL0
    pub const AP_RO_EL1: u64 = 2 << 6; // RO at EL1, no access at EL0
    pub const AP_RO_ALL: u64 = 3 << 6; // RO at EL1 and EL0

    /// Execute never bits
    pub const UXN: u64 = 1 << 54; // User execute never
    pub const PXN: u64 = 1 << 53; // Privileged execute never

    /// Memory attribute index (bits 4:2)
    pub const fn attr_index(idx: u8) -> u64 {
        ((idx as u64) & 0x7) << 2
    }

    /// Common combinations
    pub const KERNEL_RWX: u64 = VALID | PAGE | AF | SH_INNER | AP_RW_EL1 | attr_index(mair::NORMAL_WB);
    pub const KERNEL_RW: u64 = KERNEL_RWX | PXN | UXN;
    pub const KERNEL_RO: u64 = VALID | PAGE | AF | SH_INNER | AP_RO_EL1 | attr_index(mair::NORMAL_WB) | PXN | UXN;
    pub const KERNEL_RX: u64 = VALID | PAGE | AF | SH_INNER | AP_RO_EL1 | attr_index(mair::NORMAL_WB) | UXN;

    pub const USER_RWX: u64 = VALID | PAGE | AF | SH_INNER | AP_RW_ALL | attr_index(mair::NORMAL_WB);
    pub const USER_RW: u64 = USER_RWX | PXN | UXN;
    pub const USER_RO: u64 = VALID | PAGE | AF | SH_INNER | AP_RO_ALL | attr_index(mair::NORMAL_WB) | PXN | UXN;
    pub const USER_RX: u64 = VALID | PAGE | AF | SH_INNER | AP_RO_ALL | attr_index(mair::NORMAL_WB) | PXN;

    pub const DEVICE: u64 = VALID | PAGE | AF | AP_RW_EL1 | attr_index(mair::DEVICE_NGNRNE) | PXN | UXN;

    use super::mair;
}

/// Align address down to page boundary
pub const fn page_align_down(addr: usize) -> usize {
    addr & PAGE_MASK
}

/// Align address up to page boundary
pub const fn page_align_up(addr: usize) -> usize {
    (addr + PAGE_SIZE - 1) & PAGE_MASK
}

/// Convert physical address to kernel virtual address (via direct map)
pub const fn phys_to_virt(phys: usize) -> usize {
    kernel::PHYS_MAP_BASE + phys
}

/// Convert kernel virtual address to physical address (via direct map)
pub const fn virt_to_phys(virt: usize) -> usize {
    virt - kernel::PHYS_MAP_BASE
}

/// Check if address is in kernel space
pub const fn is_kernel_addr(addr: usize) -> bool {
    addr >= kernel::BASE
}

/// Check if address is in user space
pub const fn is_user_addr(addr: usize) -> bool {
    addr < user::END
}

/// Extract page table indices from virtual address
pub const fn va_indices(va: usize) -> (usize, usize, usize, usize) {
    let l0 = (va >> L0_SHIFT) & 0x1FF;
    let l1 = (va >> L1_SHIFT) & 0x1FF;
    let l2 = (va >> L2_SHIFT) & 0x1FF;
    let l3 = (va >> L3_SHIFT) & 0x1FF;
    (l0, l1, l2, l3)
}
