//! Get started by obtaining an [`Hpet`] using [`Hpet::new`].
#![no_std]
#![feature(debug_closure_helpers)]
#![warn(clippy::undocumented_unsafe_blocks)]
mod hpet;
mod mmio;

pub use arbitrary_int;
pub use hpet::*;
pub use mmio::*;
