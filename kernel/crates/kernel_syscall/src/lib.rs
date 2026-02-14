#![no_std]
#![feature(negative_impls)]
extern crate alloc;

pub mod access;
pub mod fcntl;
pub mod malloc;
pub mod mman;
pub mod stat;
pub mod unistd;

mod ptr;
pub use ptr::*;
