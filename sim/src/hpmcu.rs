//! Simulator of the RV1106 HPMCU (Syntacore SCR1) watchdog firmware.
//!
//! A faithful software port of `flare-edge/hpmcu/watchdog/main.c`'s poll loop:
//! it reads the Linux-owned mailbox words (magic + heartbeat counter), writes the
//! MCU-owned state word, and fires (records a CRU reset) on the same deadlines:
//! boot-grace if userspace never arms it, heartbeat-timeout if a live heartbeat
//! stops. Because it runs on a [`MemBus`], the *same* flared arm/beat protocol can
//! be driven against it in a host unit test, with a virtual clock, in
//! microseconds: the validation that was missing when a boot-loaded build of this
//! firmware had to be tested by flashing a panel.
//!
//! Deadlines are modelled in whole seconds (the firmware's cycle math exists only
//! to convert the core clock to these wall-clock seconds); a `tick(now_secs)` runs
//! exactly one iteration of the firmware's `for(;;)` body at that virtual time.

use crate::membus::MemBus;

pub const MB_BASE: u64 = 0xff6f_ff00;
const OFF_MAGIC: u64 = 0x00; // Linux-owned
const OFF_COUNTER: u64 = 0x04; // Linux-owned
const OFF_STATE: u64 = 0x08; // MCU-owned
const OFF_SEEN: u64 = 0x0c; // MCU-owned
const OFF_CYC: u64 = 0x10; // MCU-owned (coarse runtime)

pub const MAGIC_ARMED: u32 = 0x5741_5244; // "WARD"
pub const MAGIC_DISARM: u32 = 0x4449_5341; // "DISA"

pub const STATE_BOOT: u32 = 0xB007_0000;
pub const STATE_ARMED: u32 = 0xA07D_0000;
pub const STATE_DISARMED: u32 = 0xD15A_0000;
pub const STATE_FIRED: u32 = 0xF17E_0000;

pub const HEARTBEAT_TIMEOUT_S: u64 = 90;
pub const BOOT_GRACE_S: u64 = 300;

/// The simulated MCU core running the watchdog firmware against `bus`.
pub struct HpmcuSim<B: MemBus> {
    bus: B,
    base: u64,
    t0: u64,
    last_change: u64,
    last_seen: u32,
    polls: u32,
    armed_ever: bool,
    fired: bool,
    fire_count: u64,
}

impl<B: MemBus> HpmcuSim<B> {
    /// Bring the MCU up at `start_now` (models the firmware's `main()` prologue:
    /// state=BOOT, seen=0, boot-grace clock starts here).
    pub fn new(bus: B, base: u64, start_now: u64) -> Self {
        bus.poke32(base + OFF_STATE, STATE_BOOT);
        bus.poke32(base + OFF_SEEN, 0);
        Self {
            bus,
            base,
            t0: start_now,
            last_change: start_now,
            last_seen: 0,
            polls: 0,
            armed_ever: false,
            fired: false,
            fire_count: 0,
        }
    }

    pub fn fired(&self) -> bool {
        self.fired
    }
    pub fn fire_count(&self) -> u64 {
        self.fire_count
    }
    /// The sentinel half (high 16 bits) of the MCU state word.
    pub fn state(&self) -> u32 {
        self.bus.peek32(self.base + OFF_STATE) & 0xffff_0000
    }

    fn fire(&mut self) {
        self.bus.poke32(self.base + OFF_STATE, STATE_FIRED);
        self.fired = true;
        self.fire_count += 1;
        // Real firmware then writes the CRU global reset and spins forever; a
        // harness modelling the resulting reboot re-creates the MCU (new boot
        // grace). Here we just record that it fired.
    }

    /// One iteration of the firmware poll loop at virtual time `now` (seconds).
    /// Once fired, the real core spins in `fire()`, so further ticks are no-ops.
    pub fn tick(&mut self, now: u64) {
        if self.fired {
            return;
        }
        self.polls = self.polls.wrapping_add(1);
        self.bus
            .poke32(self.base + OFF_CYC, (now & 0xffff_ffff) as u32);
        let magic = self.bus.peek32(self.base + OFF_MAGIC);
        let counter = self.bus.peek32(self.base + OFF_COUNTER);
        let poll_lo = self.polls & 0xffff;

        if magic == MAGIC_DISARM {
            // Deliberate stand-down: resume the moment Linux re-arms; the grace
            // clock restarts so a disarm-then-silence never fires.
            self.bus
                .poke32(self.base + OFF_STATE, STATE_DISARMED | poll_lo);
            self.t0 = now;
            self.last_change = now;
        } else if magic == MAGIC_ARMED {
            self.armed_ever = true;
            if counter != self.last_seen {
                self.last_seen = counter;
                self.bus.poke32(self.base + OFF_SEEN, counter);
                self.last_change = now;
            }
            self.bus
                .poke32(self.base + OFF_STATE, STATE_ARMED | poll_lo);
            if now.saturating_sub(self.last_change) > HEARTBEAT_TIMEOUT_S {
                self.fire();
            }
        } else {
            // Not (yet) armed. Catches "kernel never brought userspace up" via
            // boot-grace, and a live-then-dead heartbeat via the timeout.
            self.bus.poke32(self.base + OFF_STATE, STATE_BOOT | poll_lo);
            if !self.armed_ever && now.saturating_sub(self.t0) > BOOT_GRACE_S {
                self.fire();
            }
            if self.armed_ever && now.saturating_sub(self.last_change) > HEARTBEAT_TIMEOUT_S {
                self.fire();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::membus::SimBus;

    fn mcu(bus: &SimBus) -> HpmcuSim<SimBus> {
        HpmcuSim::new(bus.clone(), MB_BASE, 0)
    }
    fn arm_beat(bus: &SimBus, counter: u32) {
        bus.poke32(MB_BASE + OFF_COUNTER, counter);
        bus.poke32(MB_BASE + OFF_MAGIC, MAGIC_ARMED);
    }

    #[test]
    fn boots_into_boot_state() {
        let bus = SimBus::new();
        let m = mcu(&bus);
        assert_eq!(m.state(), STATE_BOOT);
        assert!(!m.fired());
    }

    #[test]
    fn boot_grace_fires_only_after_300s_when_never_armed() {
        let bus = SimBus::new();
        let mut m = mcu(&bus);
        // Poll steadily; magic stays 0 (never armed).
        for now in (0..=300).step_by(10) {
            m.tick(now);
            assert!(!m.fired(), "must not fire at or before boot-grace ({now}s)");
        }
        m.tick(301);
        assert!(m.fired(), "must fire just past the 300s boot grace");
        assert_eq!(m.state(), STATE_FIRED);
    }

    #[test]
    fn armed_and_beating_never_fires() {
        let bus = SimBus::new();
        let mut m = mcu(&bus);
        // flared arms + beats every 5s for a simulated hour.
        for (i, now) in (0..3600).step_by(5).enumerate() {
            arm_beat(&bus, i as u32 + 1);
            m.tick(now);
            assert!(!m.fired(), "a beating heartbeat must never fire (@{now}s)");
        }
        assert_eq!(m.state(), STATE_ARMED);
    }

    #[test]
    fn stops_beating_fires_after_heartbeat_timeout() {
        let bus = SimBus::new();
        let mut m = mcu(&bus);
        arm_beat(&bus, 1);
        m.tick(0);
        assert_eq!(m.state(), STATE_ARMED);
        // Heartbeat stops (counter frozen). Fires just past 90s.
        for now in (5..=90).step_by(5) {
            m.tick(now);
            assert!(
                !m.fired(),
                "must not fire within the heartbeat window (@{now}s)"
            );
        }
        m.tick(91);
        assert!(m.fired(), "must fire just past the 90s heartbeat timeout");
    }

    #[test]
    fn disarm_stands_down_indefinitely() {
        let bus = SimBus::new();
        let mut m = mcu(&bus);
        bus.poke32(MB_BASE + OFF_MAGIC, MAGIC_DISARM);
        // Silence for well past both deadlines: a disarmed MCU never fires.
        for now in (0..=1000).step_by(10) {
            m.tick(now);
            assert!(!m.fired(), "a disarmed MCU must never fire (@{now}s)");
        }
        assert_eq!(m.state(), STATE_DISARMED);
    }

    #[test]
    fn flared_arms_within_boot_grace_no_boot_loop() {
        // The exact safety property the boot-loaded watchdog needs: on a healthy
        // boot, flared comes up well before the 300s boot grace, arms the MCU, and
        // keeps beating, so it transitions BOOT -> ARMED and never fires. (A
        // failure here would be the boot-loop we must never ship.)
        let bus = SimBus::new();
        let mut m = mcu(&bus);
        // 0..40s: kernel booting, flared not up yet (magic 0). MCU polls in BOOT.
        for now in (0..40).step_by(5) {
            m.tick(now);
            assert_eq!(m.state(), STATE_BOOT);
            assert!(!m.fired());
        }
        // flared starts at 40s and arms+beats every 5s thereafter.
        let mut counter = 0u32;
        for now in (40..1000).step_by(5) {
            counter += 1;
            arm_beat(&bus, counter);
            m.tick(now);
            assert!(
                !m.fired(),
                "flared armed before boot-grace: must never fire (@{now}s)"
            );
        }
        assert_eq!(m.state(), STATE_ARMED);
    }

    #[test]
    fn flared_dies_after_arming_fires_and_would_roll_back() {
        // flared arms, runs a while, then crashes (stops beating). The MCU fires
        // after the heartbeat timeout -> reset -> (with A/B) rollback. This is the
        // recovery the whole supervisor exists for.
        let bus = SimBus::new();
        let mut m = mcu(&bus);
        let mut counter = 0u32;
        // Beat through t=600 (inclusive): the last heartbeat lands at 600s.
        for now in (0..=600).step_by(5) {
            counter += 1;
            arm_beat(&bus, counter);
            m.tick(now);
        }
        assert!(!m.fired());
        // flared is gone: counter frozen, magic still ARMED. last_change=600, so
        // the 90s heartbeat window closes at 690s; it must not fire before then.
        for now in (605..=690).step_by(5) {
            m.tick(now);
            assert!(!m.fired(), "within the heartbeat window (@{now}s)");
        }
        m.tick(695); // 695 - 600 = 95 > 90
        assert!(
            m.fired(),
            "flared dead > heartbeat timeout: MCU fires the reset"
        );
    }
}
