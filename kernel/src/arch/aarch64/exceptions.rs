use core::arch::asm;

/// Exception vector table
#[repr(C, align(2048))]
pub struct ExceptionVectorTable {
    // Current EL with SP0
    curr_el_sp0_sync: [u8; 128],
    curr_el_sp0_irq: [u8; 128],
    curr_el_sp0_fiq: [u8; 128],
    curr_el_sp0_serror: [u8; 128],

    // Current EL with SPx
    curr_el_spx_sync: [u8; 128],
    curr_el_spx_irq: [u8; 128],
    curr_el_spx_fiq: [u8; 128],
    curr_el_spx_serror: [u8; 128],

    // Lower EL using AArch64
    lower_el_aarch64_sync: [u8; 128],
    lower_el_aarch64_irq: [u8; 128],
    lower_el_aarch64_fiq: [u8; 128],
    lower_el_aarch64_serror: [u8; 128],

    // Lower EL using AArch32
    lower_el_aarch32_sync: [u8; 128],
    lower_el_aarch32_irq: [u8; 128],
    lower_el_aarch32_fiq: [u8; 128],
    lower_el_aarch32_serror: [u8; 128],
}

/// Initialize exception vector table
pub fn init_exception_vector() {
    unsafe {
        let vbar = exception_vector_base as *const () as u64;
        asm!("msr vbar_el1, {}", in(reg) vbar);
    }
}

/// Exception vector base (defined in assembly)
unsafe extern "C" {
    fn exception_vector_base();
}

/// Synchronous exception handler
#[unsafe(no_mangle)]
pub extern "C" fn handle_sync_exception() {
    let esr: u64;
    let elr: u64;
    let far: u64;

    unsafe {
        asm!("mrs {}, esr_el1", out(reg) esr);
        asm!("mrs {}, elr_el1", out(reg) elr);
        asm!("mrs {}, far_el1", out(reg) far);
    }

    let ec = (esr >> 26) & 0x3F; // Exception class
    let iss = esr & 0x1FFFFFF; // Instruction specific syndrome

    match ec {
        0x15 => {
            // SVC instruction execution in AArch64 state
            crate::arch::aarch64::syscall::handle_syscall();
        }
        0x20 | 0x21 => {
            // Instruction abort from lower/same EL
            panic!("Instruction abort at {:#x}, far: {:#x}", elr, far);
        }
        0x24 | 0x25 => {
            // Data abort from lower/same EL
            handle_data_abort(elr, far, iss);
        }
        _ => {
            panic!(
                "Unhandled synchronous exception: EC={:#x}, ISS={:#x}, ELR={:#x}",
                ec, iss, elr
            );
        }
    }
}

/// Data Fault Status Code (DFSC) - bits [5:0] of ISS
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum DataFaultCode {
    // Translation faults (page not mapped)
    TranslationFaultL0 = 0b000100,
    TranslationFaultL1 = 0b000101,
    TranslationFaultL2 = 0b000110,
    TranslationFaultL3 = 0b000111,

    // Access flag faults
    AccessFlagFaultL1 = 0b001001,
    AccessFlagFaultL2 = 0b001010,
    AccessFlagFaultL3 = 0b001011,

    // Permission faults
    PermissionFaultL1 = 0b001101,
    PermissionFaultL2 = 0b001110,
    PermissionFaultL3 = 0b001111,

    // Alignment fault
    AlignmentFault = 0b100001,
}

impl DataFaultCode {
    fn from_iss(iss: u64) -> Option<Self> {
        let dfsc = (iss & 0x3F) as u8;
        match dfsc {
            0b000100 => Some(Self::TranslationFaultL0),
            0b000101 => Some(Self::TranslationFaultL1),
            0b000110 => Some(Self::TranslationFaultL2),
            0b000111 => Some(Self::TranslationFaultL3),
            0b001001 => Some(Self::AccessFlagFaultL1),
            0b001010 => Some(Self::AccessFlagFaultL2),
            0b001011 => Some(Self::AccessFlagFaultL3),
            0b001101 => Some(Self::PermissionFaultL1),
            0b001110 => Some(Self::PermissionFaultL2),
            0b001111 => Some(Self::PermissionFaultL3),
            0b100001 => Some(Self::AlignmentFault),
            _ => None,
        }
    }

    fn is_translation_fault(&self) -> bool {
        matches!(
            self,
            Self::TranslationFaultL0
                | Self::TranslationFaultL1
                | Self::TranslationFaultL2
                | Self::TranslationFaultL3
        )
    }

    fn is_permission_fault(&self) -> bool {
        matches!(
            self,
            Self::PermissionFaultL1 | Self::PermissionFaultL2 | Self::PermissionFaultL3
        )
    }
}

fn handle_data_abort(elr: u64, far: u64, iss: u64) {
    let is_write = (iss & (1 << 6)) != 0; // WnR bit
    let is_cm = (iss & (1 << 8)) != 0; // Cache maintenance
    let is_s1ptw = (iss & (1 << 7)) != 0; // Stage 1 page table walk

    let fault_code = DataFaultCode::from_iss(iss);

    log::debug!(
        "Data abort: PC={:#x}, addr={:#x}, write={}, dfsc={:?}",
        elr,
        far,
        is_write,
        fault_code
    );

    // Check if this is a kernel or user address
    let is_kernel_addr = far >= 0xFFFF_0000_0000_0000;

    match fault_code {
        Some(code) if code.is_translation_fault() => {
            // Page not mapped - this is a page fault
            if is_kernel_addr {
                // Kernel page fault - this is fatal
                panic!(
                    "Kernel page fault at PC={:#x}, address={:#x}, write={}",
                    elr, far, is_write
                );
            } else {
                // User page fault - could be demand paging
                // For now, just panic as we don't have userspace yet
                panic!(
                    "User page fault at PC={:#x}, address={:#x}, write={}",
                    elr, far, is_write
                );

                // TODO: Implement demand paging
                // 1. Check if address is in valid VMA
                // 2. Allocate physical page
                // 3. Map page with appropriate permissions
                // 4. Return to faulting instruction
            }
        }
        Some(code) if code.is_permission_fault() => {
            // Permission denied
            if is_kernel_addr {
                panic!(
                    "Kernel permission fault at PC={:#x}, address={:#x}, write={}",
                    elr, far, is_write
                );
            } else {
                // Could be copy-on-write
                panic!(
                    "User permission fault at PC={:#x}, address={:#x}, write={}",
                    elr, far, is_write
                );

                // TODO: Implement COW
                // 1. Check if this is a COW page
                // 2. If COW and write, copy page and remap as writable
                // 3. Otherwise, send SIGSEGV to process
            }
        }
        Some(DataFaultCode::AlignmentFault) => {
            panic!(
                "Alignment fault at PC={:#x}, address={:#x}",
                elr, far
            );
        }
        Some(code) => {
            // Access flag faults - need to set AF bit
            log::warn!(
                "Access flag fault {:?} at PC={:#x}, address={:#x}",
                code,
                elr,
                far
            );
            // For now, panic
            panic!("Access flag fault not yet handled");
        }
        None => {
            let dfsc = iss & 0x3F;
            panic!(
                "Unknown data abort: PC={:#x}, address={:#x}, DFSC={:#x}",
                elr, far, dfsc
            );
        }
    }
}

// IRQ handler is defined in interrupts.rs module
// (Re-exported through assembly vector table)

/// FIQ handler
#[unsafe(no_mangle)]
pub extern "C" fn handle_fiq() {
    log::warn!("FIQ received");
}

/// SError handler
#[unsafe(no_mangle)]
pub extern "C" fn handle_serror() {
    panic!("SError received");
}

/// Exception context saved on exception entry
#[repr(C)]
pub struct ExceptionContext {
    // General purpose registers
    pub x0: u64,
    pub x1: u64,
    pub x2: u64,
    pub x3: u64,
    pub x4: u64,
    pub x5: u64,
    pub x6: u64,
    pub x7: u64,
    pub x8: u64,
    pub x9: u64,
    pub x10: u64,
    pub x11: u64,
    pub x12: u64,
    pub x13: u64,
    pub x14: u64,
    pub x15: u64,
    pub x16: u64,
    pub x17: u64,
    pub x18: u64,
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64, // Frame pointer
    pub x30: u64, // Link register

    // Exception state
    pub elr: u64,  // Exception link register
    pub spsr: u64, // Saved program status register
}
