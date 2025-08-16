//! Get started by obtaining an [`Hpet`] using [`Hpet::new`].
#![no_std]
#![feature(debug_closure_helpers)]
mod hpet;
mod mmio;

pub use hpet::*;
pub use mmio::*;
