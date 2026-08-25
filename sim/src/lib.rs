//! warden-sim — host-side hardware simulator for WardenOS.
//!
//! Lets driver and supervisor logic run and be tested on the host, with no panel,
//! by modelling the RV1106 hardware the vendor SDK cannot: the register/SRAM bus
//! ([`membus`]), the RISC-V HPMCU watchdog coprocessor ([`hpmcu`]), the CRU reset
//! ladder ([`cru`]), the RS-485 device end ([`modbus`]), the **RGA** 2D blitter
//! ([`rga`], a recording `improcess` fake), and the **NPU** load surface ([`npu`],
//! the `/proc/rknpu/load` model).
//!
//! Design: one [`membus::MemBus`] seam, two backends. On the host, [`membus::SimBus`]
//! is an in-memory word map; on the device, flared's `devmem.rs` implements the same
//! trait over `/dev/mem`, so the same code runs against either. See the repo README.
//! (The [`modbus`] slave rides a byte-stream seam and [`rga`] its own call seam, not
//! the register bus.)

pub mod cru;
pub mod hpmcu;
pub mod membus;
pub mod modbus;
pub mod npu;
pub mod rga;

pub use cru::{BootMode, CruSim, ResetCause};
pub use hpmcu::HpmcuSim;
pub use membus::{MemBus, SimBus};
pub use modbus::ModbusSlave;
pub use npu::NpuSim;
pub use rga::{Blit, ImStatus, Rect, RgaSim, Surface};
