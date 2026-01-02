use core::arch::asm;

use riscv::register::{scause, sepc, sstatus, stval, stvec};

/// Initialize the trap vector
pub fn init_trap_vector() {
    unsafe {
        // Set trap vector to direct mode
        stvec::write(trap_handler as usize, stvec::TrapMode::Direct);
    }
}

/// Main trap handler entry point
#[repr(align(4))]
#[no_mangle]
pub extern "C" fn trap_handler() {
    let scause = scause::read();
    let sepc = sepc::read();
    let stval = stval::read();

    match scause.cause() {
        scause::Trap::Exception(exception) => {
            handle_exception(exception, sepc, stval);
        }
        scause::Trap::Interrupt(interrupt) => {
            handle_interrupt(interrupt);
        }
    }
}

fn handle_exception(exception: scause::Exception, sepc: usize, stval: usize) {
    use scause::Exception;

    match exception {
        Exception::InstructionMisaligned => {
            panic!("Instruction misaligned at {:#x}, stval: {:#x}", sepc, stval);
        }
        Exception::InstructionFault => {
            panic!("Instruction fault at {:#x}, stval: {:#x}", sepc, stval);
        }
        Exception::IllegalInstruction => {
            panic!("Illegal instruction at {:#x}, stval: {:#x}", sepc, stval);
        }
        Exception::Breakpoint => {
            log::warn!("Breakpoint at {:#x}", sepc);
            // Advance past the breakpoint instruction
            unsafe {
                sepc::write(sepc + 2); // ebreak is 2 bytes in compressed mode
            }
        }
        Exception::LoadFault => {
            panic!("Load fault at {:#x}, address: {:#x}", sepc, stval);
        }
        Exception::StoreMisaligned => {
            panic!("Store misaligned at {:#x}, address: {:#x}", sepc, stval);
        }
        Exception::StoreFault => {
            panic!("Store fault at {:#x}, address: {:#x}", sepc, stval);
        }
        Exception::UserEnvCall => {
            // Handle syscall from user mode
            crate::arch::riscv64::syscall::handle_syscall();
        }
        Exception::InstructionPageFault => {
            handle_page_fault(sepc, stval, false);
        }
        Exception::LoadPageFault => {
            handle_page_fault(sepc, stval, false);
        }
        Exception::StorePageFault => {
            handle_page_fault(sepc, stval, true);
        }
        _ => {
            panic!("Unhandled exception: {:?} at {:#x}", exception, sepc);
        }
    }
}

fn handle_interrupt(interrupt: scause::Interrupt) {
    use scause::Interrupt;

    match interrupt {
        Interrupt::SupervisorSoft => {
            // Software interrupt (IPI)
            crate::arch::riscv64::interrupts::handle_soft_interrupt();
        }
        Interrupt::SupervisorTimer => {
            // Timer interrupt
            crate::arch::riscv64::interrupts::handle_timer_interrupt();
        }
        Interrupt::SupervisorExternal => {
            // External interrupt (PLIC)
            crate::arch::riscv64::interrupts::handle_external_interrupt();
        }
        _ => {
            log::warn!("Unhandled interrupt: {:?}", interrupt);
        }
    }
}

fn handle_page_fault(sepc: usize, stval: usize, is_write: bool) {
    log::error!(
        "Page fault at {:#x}, address: {:#x}, write: {}",
        sepc,
        stval,
        is_write
    );

    // TODO: Implement lazy page allocation and COW
    // For now, just panic
    panic!("Page fault not yet implemented");
}

/// Context saved on trap entry
#[repr(C)]
pub struct TrapFrame {
    pub ra: usize,  // x1: return address
    pub sp: usize,  // x2: stack pointer
    pub gp: usize,  // x3: global pointer
    pub tp: usize,  // x4: thread pointer
    pub t0: usize,  // x5: temporary
    pub t1: usize,  // x6: temporary
    pub t2: usize,  // x7: temporary
    pub s0: usize,  // x8: saved register / frame pointer
    pub s1: usize,  // x9: saved register
    pub a0: usize,  // x10: argument / return value
    pub a1: usize,  // x11: argument / return value
    pub a2: usize,  // x12: argument
    pub a3: usize,  // x13: argument
    pub a4: usize,  // x14: argument
    pub a5: usize,  // x15: argument
    pub a6: usize,  // x16: argument
    pub a7: usize,  // x17: argument
    pub s2: usize,  // x18: saved register
    pub s3: usize,  // x19: saved register
    pub s4: usize,  // x20: saved register
    pub s5: usize,  // x21: saved register
    pub s6: usize,  // x22: saved register
    pub s7: usize,  // x23: saved register
    pub s8: usize,  // x24: saved register
    pub s9: usize,  // x25: saved register
    pub s10: usize, // x26: saved register
    pub s11: usize, // x27: saved register
    pub t3: usize,  // x28: temporary
    pub t4: usize,  // x29: temporary
    pub t5: usize,  // x30: temporary
    pub t6: usize,  // x31: temporary
}
