//! warden-sim — host-side hardware simulator for WardenOS.
//!
//! Lets driver and supervisor logic run and be tested on the host, with no panel,
//! by modelling the RV1106 hardware the vendor SDK cannot: the register/SRAM bus
//! ([`membus`]), the RISC-V HPMCU watchdog coprocessor ([`hpmcu`]), and — as they
//! land — the RGA blitter and the NPU.
//!
//! Design: one [`membus::MemBus`] seam, two backends. On the host, [`membus::SimBus`]
//! is an in-memory word map; on the device, flared's `devmem.rs` implements the same
//! trait over `/dev/mem`, so the same code runs against either. See the repo README.

pub mod hpmcu;
pub mod membus;

pub use hpmcu::HpmcuSim;
pub use membus::{MemBus, SimBus};
