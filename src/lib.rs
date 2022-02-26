#![no_std]
// #![feature(async_closure)]
#![feature(asm)]
#![feature(maybe_uninit_extra)]
#![feature(drain_filter)]

extern crate alloc;

pub use mutex::*;
mod interrupt;
pub mod mutex;
pub mod spinlock;
pub mod rwlock;
mod waiter;
