//! Single-Source eBPF Kernel with Build-Time Physical Profiles
//!
//! This crate implements a profile-constrained eBPF subsystem where the same source code
//! is used for both cloud and embedded deployments. The difference between profiles is
//! expressed through build-time selection and compile-time erasure, not code forks.
//!
//! # Profiles
//!
//! - **Cloud Profile**: Elastic resources, JIT compilation, soft latency bounds
//! - **Embedded Profile**: Static resources, interpreter/AOT only, hard deadline enforcement
//!
//! # Usage
//!
//! Select a profile at build time:
//! ```bash
//! cargo build --features cloud-profile
//! # or
//! cargo build --features embedded-profile
//! ```
//!
//! # Guiding Principle
//!
//! > "Cloud and embedded share one source code; the difference is not features,
//! > but the physical assumptions the kernel is allowed to make at build time."

#![no_std]

extern crate alloc;

// Compile-time mutual exclusion: exactly one profile must be selected
#[cfg(all(feature = "cloud-profile", feature = "embedded-profile"))]
compile_error!(
    "Cannot enable both `cloud-profile` and `embedded-profile` features simultaneously. \
     Select exactly one profile at build time."
);

#[cfg(not(any(feature = "cloud-profile", feature = "embedded-profile")))]
compile_error!(
    "Must enable either `cloud-profile` or `embedded-profile` feature. \
     Use `--features cloud-profile` or `--features embedded-profile` when building."
);

pub mod bytecode;
pub mod execution;
pub mod maps;
pub mod profile;
pub mod scheduler;
pub mod verifier;
