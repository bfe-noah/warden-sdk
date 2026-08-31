//! NPU load model: the `/proc/rknpu/load` surface.
//!
//! The rknpu driver exposes utilisation at `/proc/rknpu/load` as `"NPU load:  N%"`,
//! and the file exists only once `rknpu.ko` is loaded, so a *missing* file means
//! the driver is absent, not idle (sysmon reports absent as 0 and labels the
//! screen). This models both a present NPU at a chosen load and an absent one, and
//! mirrors sysmon's parse (`strchr(buf, ':')` then the leading integer) so the
//! driver's reader can be exercised against realistic text.
//!
//! (Only `/proc/rknpu/load` is modelled. `/proc/rknpu/volt` is deliberately NOT:
//! reading it SIGSEGVs the reader on this board, so no code should ever open it.)

/// A modelled NPU. `present == false` models rknpu.ko not loaded (no proc file).
#[derive(Clone, Copy, Debug)]
pub struct NpuSim {
    present: bool,
    load: u8,
}

impl NpuSim {
    /// A present NPU reporting 0% load.
    pub fn new() -> Self {
        Self {
            present: true,
            load: 0,
        }
    }

    /// An absent NPU (rknpu.ko not loaded): `/proc/rknpu/load` does not exist.
    pub fn absent() -> Self {
        Self {
            present: false,
            load: 0,
        }
    }

    /// Set the reported load, clamped to 0..=100.
    pub fn set_load(&mut self, pct: u8) {
        self.load = if pct > 100 { 100 } else { pct };
    }

    /// Contents of `/proc/rknpu/load`, or `None` when the NPU is absent (the file
    /// would not exist, so the reader fails soft to "NPU absent").
    pub fn proc_load(&self) -> Option<String> {
        if self.present {
            Some(format!("NPU load:  {}%\n", self.load))
        } else {
            None
        }
    }

    /// Parse a load percent out of a `/proc/rknpu/load` line the way sysmon does:
    /// the text after the first ':' then the leading integer. `None` if there is
    /// no colon or no number (which the driver treats as "report 0 / absent").
    pub fn parse_load(text: &str) -> Option<u8> {
        let after = text.split_once(':')?.1;
        let digits: String = after
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse::<u8>().ok()
    }
}

impl Default for NpuSim {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_reports_load_and_round_trips() {
        let mut n = NpuSim::new();
        n.set_load(42);
        let s = n.proc_load().unwrap();
        assert_eq!(s, "NPU load:  42%\n");
        assert_eq!(NpuSim::parse_load(&s), Some(42));
    }

    #[test]
    fn load_clamps_to_100() {
        let mut n = NpuSim::new();
        n.set_load(250);
        assert_eq!(NpuSim::parse_load(&n.proc_load().unwrap()), Some(100));
    }

    #[test]
    fn absent_has_no_proc_file() {
        assert!(NpuSim::absent().proc_load().is_none());
    }

    #[test]
    fn parse_rejects_no_colon() {
        assert_eq!(NpuSim::parse_load("no colon here"), None);
    }

    #[test]
    fn parse_rejects_non_numeric() {
        assert_eq!(NpuSim::parse_load("NPU load:  x%"), None);
    }

    #[test]
    fn parse_handles_zero() {
        assert_eq!(NpuSim::parse_load("NPU load:  0%"), Some(0));
    }

    #[test]
    fn default_is_present_idle() {
        assert_eq!(
            NpuSim::default().proc_load(),
            Some("NPU load:  0%\n".to_string())
        );
    }
}
