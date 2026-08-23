//! The register / SRAM access seam.
//!
//! Every hardware block on the RV1106 that our code touches through `/dev/mem`
//! (the CRU reset ladder, the HPMCU mailbox, the SGRF boot-addr register) reaches
//! it as a 32-bit poke or peek at a physical address. `MemBus` is that operation,
//! abstracted so the same driver/supervisor code runs against either:
//!
//!   * the **real** backend — an mmap of `/dev/mem` (lives in flared's
//!     `devmem.rs`; it will implement this trait so its logic is host-testable), or
//!   * the **sim** backend — [`SimBus`], an in-memory word map.
//!
//! `SimBus` is `Clone` + internally `Arc<Mutex<..>>`, so the simulated MCU core
//! and the "Linux side" can each hold a handle and read/write the *same* shared
//! memory — exactly the two-core mailbox the real system uses — with no
//! cache-maintenance dance to model (the real mailbox sits in the GRF uncached
//! window).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A 32-bit physical-address register/SRAM bus. Addresses must be 4-byte aligned.
pub trait MemBus {
    fn peek32(&self, phys: u64) -> u32;
    fn poke32(&self, phys: u64, val: u32);
}

/// In-memory bus for host tests. Unwritten words read as 0. Shared handles
/// (via `clone`) alias the same backing store.
#[derive(Clone, Default)]
pub struct SimBus {
    words: Arc<Mutex<HashMap<u64, u32>>>,
}

impl SimBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot every written word (address-sorted) — for test assertions/dumps.
    pub fn dump(&self) -> Vec<(u64, u32)> {
        let g = self.words.lock().unwrap();
        let mut v: Vec<(u64, u32)> = g.iter().map(|(&a, &w)| (a, w)).collect();
        v.sort_by_key(|&(a, _)| a);
        v
    }
}

impl MemBus for SimBus {
    fn peek32(&self, phys: u64) -> u32 {
        debug_assert_eq!(phys & 0x3, 0, "unaligned peek32 @ {phys:#x}");
        *self.words.lock().unwrap().get(&phys).unwrap_or(&0)
    }

    fn poke32(&self, phys: u64, val: u32) {
        debug_assert_eq!(phys & 0x3, 0, "unaligned poke32 @ {phys:#x}");
        self.words.lock().unwrap().insert(phys, val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwritten_reads_zero_and_writes_alias_through_clones() {
        let a = SimBus::new();
        let b = a.clone();
        assert_eq!(a.peek32(0xff6f_ff00), 0);
        a.poke32(0xff6f_ff00, 0xdead_beef);
        // The clone sees it: same shared store (two-core shared memory).
        assert_eq!(b.peek32(0xff6f_ff00), 0xdead_beef);
        assert_eq!(a.dump(), vec![(0xff6f_ff00, 0xdead_beef)]);
    }
}
