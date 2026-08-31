//! CRU reset-ladder model on the [`MemBus`] seam.
//!
//! `reboot -f` does NOT reset the RV1106 (no PSCI/restart handler). The canonical
//! reset is the CRU global-first software reset (rung 1), with the DesignWare
//! watchdog as a backstop (rung 2): the ladder in flared's `devmem::hard_reset`.
//! This model lets that ladder, and the boot-mode -> MaskRom recovery maneuver, be
//! exercised entirely on the host: run the pokes against a [`SimBus`], then
//! [`CruSim::poll`] to see which rung fired and what boot mode a warm reset lands in.
//!
//! It bakes in the two hardware facts that cost real hardware time:
//!   * the CRU global-reset register is `0xff3b0c08` magic `0xfdb9`: the offset
//!     `0xff3a0614` from *other* Rockchip SoCs is a **silent no-op** here (the model
//!     ignores it, so a regression that reverts to the wrong offset fails a test);
//!   * the boot-mode register `0xff020200` **survives a warm reset** and is cleared
//!     only by a power-on reset: the mechanism that makes "set MaskRom, then reset"
//!     drop the SoC into BootROM download without the BOOT button.

use crate::membus::MemBus;

/// Correct RV1106 global-first software reset (confirmed on hardware 2026-08-14).
pub const CRU_GLB_SRST_FST: u64 = 0xff3b_0c08;
pub const CRU_GLB_SRST_MAGIC: u32 = 0xfdb9;
/// Wrong offset carried over from other Rockchip SoCs: a silent no-op on RV1106.
pub const CRU_WRONG_OFFSET: u64 = 0xff3a_0614;

/// DesignWare watchdog (rung 2 backstop).
pub const DW_WDT_BASE: u64 = 0xff5a_0000;
pub const DW_WDT_CR: u64 = DW_WDT_BASE; // bit0 = enable
pub const DW_WDT_TORR: u64 = DW_WDT_BASE + 0x4; // timeout range select
pub const DW_WDT_CRR: u64 = DW_WDT_BASE + 0xc; // write 0x76 to pet
pub const DW_WDT_PET: u32 = 0x76;
pub const DW_WDT_EN: u32 = 0x1;

/// Boot-mode register: survives warm reset, cleared by POR.
pub const BOOT_MODE_REG: u64 = 0xff02_0200;
pub const BOOT_NORMAL: u32 = 0x5242_c300;
pub const BOOT_LOADER: u32 = 0x5242_c301; // U-Boot rockusb download
pub const BOOT_MASKROM: u32 = 0xef08_a53c; // BootROM MaskRom (db-able)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResetCause {
    Cru,
    Watchdog,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BootMode {
    Normal,
    Loader,
    Maskrom,
}

impl BootMode {
    fn from_reg(v: u32) -> BootMode {
        match v {
            BOOT_LOADER => BootMode::Loader,
            BOOT_MASKROM => BootMode::Maskrom,
            _ => BootMode::Normal, // unknown/empty/BOOT_NORMAL all boot normally
        }
    }
}

/// Models the reset ladder + boot-mode register over a [`MemBus`]. Poll it after
/// running reset code against the same bus.
pub struct CruSim<B: MemBus> {
    bus: B,
    reset_count: u64,
    last_cause: Option<ResetCause>,
    boot_mode: BootMode,
    /// WDT timeout in `now`-ticks (derived from TORR on enable). None = disabled.
    wdt_deadline: Option<u64>,
}

impl<B: MemBus> CruSim<B> {
    pub fn new(bus: B) -> Self {
        Self {
            bus,
            reset_count: 0,
            last_cause: None,
            boot_mode: BootMode::Normal,
            wdt_deadline: None,
        }
    }

    pub fn reset_count(&self) -> u64 {
        self.reset_count
    }
    pub fn last_cause(&self) -> Option<ResetCause> {
        self.last_cause
    }
    /// Boot mode the most recent (warm) reset landed in. MaskRom/Loader stick until
    /// a power-on reset; a plain reset with no boot-mode set lands in Normal.
    pub fn boot_mode(&self) -> BootMode {
        self.boot_mode
    }

    /// A power-on reset: clears the boot-mode register (the one thing a warm reset
    /// preserves), so the next boot is Normal regardless of a stale MaskRom request.
    pub fn power_on_reset(&mut self) {
        self.bus.poke32(BOOT_MODE_REG, BOOT_NORMAL);
        self.boot_mode = BootMode::Normal;
        self.wdt_deadline = None;
    }

    /// Advance the model to time `now` and apply any pending reset. Returns the
    /// cause if a reset fired this tick. A reset consumes its trigger (the CRU
    /// magic / the WDT deadline) and reads the *preserved* boot-mode register.
    pub fn poll(&mut self, now: u64) -> Option<ResetCause> {
        // Rung 1: CRU global-first software reset. ONLY the correct register fires;
        // the magic at the wrong offset is a silent no-op (the register just holds
        // the value and nothing happens).
        if self.bus.peek32(CRU_GLB_SRST_FST) == CRU_GLB_SRST_MAGIC {
            self.bus.poke32(CRU_GLB_SRST_FST, 0); // reset consumes the request
            return Some(self.fire(ResetCause::Cru));
        }

        // Rung 2: DesignWare watchdog. Enabled (CR bit0) + deadline passed with no
        // intervening pet => reset.
        let cr = self.bus.peek32(DW_WDT_CR);
        if cr & DW_WDT_EN != 0 {
            // A pet (CRR == 0x76) rearms the timer; consume it so we detect the next.
            if self.bus.peek32(DW_WDT_CRR) == DW_WDT_PET {
                self.bus.poke32(DW_WDT_CRR, 0);
                self.arm_wdt(now);
            } else if self.wdt_deadline.is_none() {
                self.arm_wdt(now); // just enabled: start the timer
            }
            if let Some(deadline) = self.wdt_deadline {
                if now >= deadline {
                    self.wdt_deadline = None;
                    return Some(self.fire(ResetCause::Watchdog));
                }
            }
        } else {
            self.wdt_deadline = None;
        }
        None
    }

    fn arm_wdt(&mut self, now: u64) {
        // TORR selects the timeout; model it as 1<<torr ticks (monotonic in TORR,
        // torr=0 => 1 tick so the reset-ladder backstop fires promptly).
        let torr = self.bus.peek32(DW_WDT_TORR) & 0xf;
        self.wdt_deadline = Some(now + (1u64 << torr));
    }

    fn fire(&mut self, cause: ResetCause) -> ResetCause {
        self.reset_count += 1;
        self.last_cause = Some(cause);
        // A warm reset preserves the boot-mode register: that is exactly how
        // "poke MaskRom then reset" reaches BootROM download.
        self.boot_mode = BootMode::from_reg(self.bus.peek32(BOOT_MODE_REG));
        cause
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::membus::SimBus;

    /// Rung 1: the correct CRU global-reset register + magic fires a CRU reset.
    #[test]
    fn cru_global_reset_fires() {
        let bus = SimBus::new();
        let mut cru = CruSim::new(bus.clone());
        bus.poke32(CRU_GLB_SRST_FST, CRU_GLB_SRST_MAGIC);
        assert_eq!(cru.poll(0), Some(ResetCause::Cru));
        assert_eq!(cru.reset_count(), 1);
        assert_eq!(cru.boot_mode(), BootMode::Normal);
    }

    /// The hardware lesson as a regression: the magic at the WRONG offset does
    /// nothing (a silent no-op), so the ladder must fall through to the watchdog.
    #[test]
    fn wrong_offset_is_a_silent_noop() {
        let bus = SimBus::new();
        let mut cru = CruSim::new(bus.clone());
        bus.poke32(CRU_WRONG_OFFSET, CRU_GLB_SRST_MAGIC); // the bug
        assert_eq!(cru.poll(0), None, "wrong-offset write must NOT reset");
        assert_eq!(cru.reset_count(), 0);
    }

    /// Rung 2: enabling the DW watchdog and not petting it past the deadline fires.
    #[test]
    fn watchdog_backstop_fires_on_timeout() {
        let bus = SimBus::new();
        let mut cru = CruSim::new(bus.clone());
        bus.poke32(DW_WDT_TORR, 3); // deadline = now + 8
        bus.poke32(DW_WDT_CR, DW_WDT_EN);
        assert_eq!(cru.poll(0), None); // arms at t=0, deadline 8
        assert_eq!(cru.poll(7), None); // not yet
        assert_eq!(cru.poll(8), Some(ResetCause::Watchdog));
    }

    /// Petting the watchdog before the deadline prevents the reset.
    #[test]
    fn watchdog_pet_prevents_reset() {
        let bus = SimBus::new();
        let mut cru = CruSim::new(bus.clone());
        bus.poke32(DW_WDT_TORR, 3); // deadline window = 8 ticks
        bus.poke32(DW_WDT_CR, DW_WDT_EN);
        cru.poll(0);
        bus.poke32(DW_WDT_CRR, DW_WDT_PET); // pet at t=5 -> new deadline 13
        assert_eq!(cru.poll(5), None);
        assert_eq!(
            cru.poll(8),
            None,
            "petted: original deadline no longer applies"
        );
        assert_eq!(cru.poll(13), Some(ResetCause::Watchdog));
    }

    /// The boot-mode register survives a (warm) reset: set MaskRom, reset via CRU,
    /// and the model lands in MaskRom: the on-demand BootROM-download maneuver.
    #[test]
    fn maskrom_survives_warm_reset() {
        let bus = SimBus::new();
        let mut cru = CruSim::new(bus.clone());
        bus.poke32(BOOT_MODE_REG, BOOT_MASKROM);
        bus.poke32(CRU_GLB_SRST_FST, CRU_GLB_SRST_MAGIC);
        assert_eq!(cru.poll(0), Some(ResetCause::Cru));
        assert_eq!(cru.boot_mode(), BootMode::Maskrom);
    }

    /// A power-on reset clears the boot-mode register (unlike a warm reset), so a
    /// stale MaskRom request does not strand the device: it boots Normal.
    #[test]
    fn power_on_reset_clears_maskrom_request() {
        let bus = SimBus::new();
        let mut cru = CruSim::new(bus.clone());
        bus.poke32(BOOT_MODE_REG, BOOT_MASKROM);
        cru.power_on_reset();
        bus.poke32(CRU_GLB_SRST_FST, CRU_GLB_SRST_MAGIC);
        assert_eq!(cru.poll(0), Some(ResetCause::Cru));
        assert_eq!(cru.boot_mode(), BootMode::Normal);
    }
}
