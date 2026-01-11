//! BPF Verifier
//!
//! The verifier ensures that BPF programs are safe to execute. It performs
//! static analysis to check for:
//!
//! - Memory safety (no out-of-bounds access)
//! - Type safety (register tracking)
//! - Control flow safety (bounded loops, valid jumps)
//! - Profile-specific constraints
//!
//! # Architecture
//!
//! ```text
//!        ┌─────────────────────────┐
//!        │   Verifier Core         │
//!        │   (Safety Checks)       │
//!        └───────────┬─────────────┘
//!                    │
//!        ┌───────────┴───────────┐
//!        │                       │
//! ┌──────▼──────┐       ┌────────▼────────┐
//! │ Cloud       │       │ Embedded        │
//! │ Constraints │       │ Constraints     │
//! │ - Soft WCET │       │ - Hard WCET     │
//! │ - JIT hints │       │ - Stack ceiling │
//! └─────────────┘       │ - Interrupt safe│
//!                       │ - Energy budget │
//!                       └─────────────────┘
//! ```
//!
//! # Profile-Specific Verification
//!
//! The verifier applies different constraint checks based on the active profile:
//!
//! - **Cloud**: Relaxed constraints, JIT hints, soft WCET
//! - **Embedded**: Strict constraints, hard WCET, interrupt safety

mod cfg;
mod core;
mod error;
mod state;

pub use core::Verifier;

pub use cfg::ControlFlowGraph;
pub use error::VerifyError;
pub use state::{RegState, RegType, StackSlot, VerifierState};
