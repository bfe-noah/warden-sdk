//! config-lint: static target-config checks the behavioural sim cannot cover.
//!
//! The c8a3 brick was a memory-map fault, not a logic bug: the boot-loaded MCU's
//! load address (`0x40000`) is a `reserved-memory` carve-out on Thunder-Boot
//! boards but plain kernel RAM on ours, so the coprocessor firmware and the kernel
//! fought over the same DRAM and the board hung before eth0. No sim catches that.
//! It needs a static check against the target devicetree: **every address the
//! idblock loader drops MCU firmware to must sit inside a `reserved-memory` node.**
//!
//! This module parses the two authoritative artifacts (the rkbin loader `.ini`
//! and the built/target devicetree) and reports any MCU load that would collide.
//! Wire the CLI into CI so the mistake is caught before a flash, not on a bench.

/// One MCU/coprocessor firmware load declared by the loader `.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McuLoad {
    pub loader: String, // e.g. "LOADER2"
    pub name: String,   // e.g. "Hpmcu"
    pub load_addr: u64,
}

/// A `reserved-memory` range `[start, start+size)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: u64,
    pub size: u64,
}

impl Range {
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.start.saturating_add(self.size)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub load: McuLoad,
    pub msg: String,
}

/// A byte that can appear inside a devicetree node/property identifier: used to
/// require a token boundary before a `reg` property, so substrings like `reg-names`
/// or a `region-*` node label are not matched as the `reg` property itself.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn parse_addr(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

/// Loaders that are part of the normal RK boot chain (DDR init, SPL/miniloader, the
/// secure monitor, U-Boot) and are NOT boot-loaded coprocessor firmware, so their
/// LOAD_ADDR (if any) is the vendor boot flow, not a DRAM carve-out that must be
/// reserved. Anything NOT on this allowlist that declares a LOAD_ADDR is treated as
/// a coprocessor load and checked (**fail closed**): a future MCU named
/// "Rtos"/"Bl32"/"M0" must not slip through an MCU-*name* allowlist the way the
/// original `hpmcu`/`mcu`/`amp` substring test would have: that is exactly the
/// c8a3 0x40000-brick class this tool exists to catch.
///
/// Matched as a WHOLE normalized name (separators stripped), never a substring, so a
/// coprocessor whose vendor name merely *contains* a boot-chain word ("AudioLoader"
/// contains "loader", "SplRtos" contains "spl", "Bl32" is not "bl31") is still checked,
/// not waved through. Over-inclusion here would only over-report (the safe direction); a missed
/// coprocessor is the dangerous one.
fn is_known_safe_loader(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    const SAFE: &[&str] = &[
        "flashdata",
        "flashboot",
        "nandboot",
        "nandboot1",
        "nandboot2",
        "sdboot",
        "emmcboot",
        "spinorboot",
        "ddr",
        "usbplug",
        "spl",
        "uboot",
        "loader",
        "miniloader",
        "trust",
        "tee",
        "atf",
        "bl31",
        "fsbl",
        "idblock",
    ];
    SAFE.contains(&n.as_str())
}

/// Extract MCU firmware loads from an rkbin loader `.ini`: for each `LOADERn=<name>`
/// in `[LOADER_OPTION]` that names an MCU loader, read `LOAD_ADDR` from the matching
/// `[LOADERn_PARAM]` section.
pub fn parse_ini_mcu_loads(ini: &str) -> Vec<McuLoad> {
    let mut section = String::new();
    // loader index (e.g. "LOADER2") -> firmware name (e.g. "Hpmcu")
    let mut loaders: Vec<(String, String)> = Vec::new();
    // section name -> LOAD_ADDR
    let mut load_addrs: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    for raw in ini.lines() {
        let line = raw.split(['#', ';']).next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(sec) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = sec.trim().to_string();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if section == "LOADER_OPTION" && k.to_ascii_uppercase().starts_with("LOADER") {
            loaders.push((k.to_ascii_uppercase(), v.to_string()));
        } else if k.eq_ignore_ascii_case("LOAD_ADDR") {
            if let Some(a) = parse_addr(v) {
                load_addrs.insert(section.to_ascii_uppercase(), a);
            }
        }
    }

    let mut out = Vec::new();
    for (loader, name) in loaders {
        // Fail closed: check every loader that isn't a known-safe boot component.
        // A benign component without a LOAD_ADDR produces nothing (the `get` below
        // misses), so this only ever surfaces loads that actually target DRAM.
        if is_known_safe_loader(&name) {
            continue;
        }
        // LOADER2 -> [LOADER2_PARAM]
        if let Some(&addr) = load_addrs.get(&format!("{loader}_PARAM")) {
            out.push(McuLoad {
                loader,
                name,
                load_addr: addr,
            });
        }
    }
    out
}

/// Extract `reserved-memory` child ranges from devicetree source (a `.dts`/`.dtsi`,
/// or the flattened output of `dtc -I dtb -O dts`). Brace-tracked so only `reg`s
/// inside a `reserved-memory { ... }` node are taken. Handles `#size-cells = <1>`
/// (RV1106): each `reg = <addr size>` is one range.
pub fn parse_reserved_ranges(dt: &str) -> Vec<Range> {
    let mut out = Vec::new();
    let bytes = dt.as_bytes();
    let mut i = 0;
    while let Some(pos) = dt[i..].find("reserved-memory") {
        let mut j = i + pos;
        // find the opening brace of this node
        while j < bytes.len() && bytes[j] != b'{' {
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        let mut depth = 0i32;
        let start = j;
        while j < bytes.len() {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        j += 1;
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        // scan `reg = <addr size>` PROPERTY tokens inside [start, j). Match `reg` as
        // a whole token (identifier boundary before it, `=` after optional space),
        // so a `reg-names` property or a `region-*@...` node label (both contain the
        // substring "reg") is not misparsed into a bogus range.
        let block = &dt[start..j.min(dt.len())];
        let bb = block.as_bytes();
        let mut k = 0;
        while let Some(rel) = block[k..].find("reg") {
            let p = k + rel;
            let end = p + 3;
            k = end; // forward progress regardless of whether this "reg" matches
            let before_ok = p == 0 || !is_ident_byte(bb[p - 1]);
            let mut a = end; // first non-space byte after "reg" must be '='
            while a < bb.len() && matches!(bb[a], b' ' | b'\t' | b'\r' | b'\n') {
                a += 1;
            }
            if !(before_ok && a < bb.len() && bb[a] == b'=') {
                continue;
            }
            let Some(lt_rel) = block[a..].find('<') else {
                continue;
            };
            let lt = a + lt_rel;
            let Some(gt_rel) = block[lt..].find('>') else {
                continue;
            };
            let gt = lt + gt_rel;
            let nums: Vec<&str> = block[lt + 1..gt].split_whitespace().collect();
            if nums.len() >= 2 {
                if let (Some(addr), Some(sz)) = (parse_addr(nums[0]), parse_addr(nums[1])) {
                    out.push(Range {
                        start: addr,
                        size: sz,
                    });
                }
            }
        }
        i = j;
    }
    out
}

/// The gate: every MCU load must land inside a reserved-memory range.
pub fn check(loads: &[McuLoad], reserved: &[Range]) -> Vec<Finding> {
    loads
        .iter()
        .filter(|l| !reserved.iter().any(|r| r.contains(l.load_addr)))
        .map(|l| Finding {
            load: l.clone(),
            msg: format!(
                "{}={} loads MCU firmware at {:#x} but no reserved-memory node covers it: \
                 kernel/MCU DRAM collision (the 0x40000 brick class)",
                l.loader, l.name, l.load_addr
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The exact idblock .ini that bricked c8a3: Hpmcu at 0x40000.
    const TB_INI: &str = r#"
[LOADER_OPTION]
NUM=3
LOADER1=FlashData
LOADER2=Hpmcu
LOADER3=FlashBoot
FlashData=bin/rv11/rv1106_ddr.bin
Hpmcu=bin/rv11/rv1106_hpmcu_tb.bin
FlashBoot=bin/rv11/rv1106_spl.bin
[LOADER2_PARAM]
LOAD_ADDR=0x40000
FLAG=0x10007
"#;

    // Our board's actual loader: no Hpmcu at all.
    const NONTB_INI: &str = "[LOADER_OPTION]\nNUM=2\nLOADER1=FlashData\nLOADER2=FlashBoot\n";

    const DT_WITH_RTOS: &str = r#"
        reserved-memory {
            #address-cells = <1>;
            #size-cells = <1>;
            ranges;
            rtos@40000 { reg = <0x40000 0x3c000>; no-map; };
            ramoops@0f000000 { compatible = "ramoops"; reg = <0x0f000000 0x00100000>; };
        };
    "#;
    const DT_NO_RTOS: &str = r#"
        reserved-memory {
            ranges;
            ramoops@0f000000 { compatible = "ramoops"; reg = <0x0f000000 0x00100000>; };
        };
    "#;

    #[test]
    fn parses_hpmcu_load_addr() {
        let loads = parse_ini_mcu_loads(TB_INI);
        assert_eq!(loads.len(), 1);
        assert_eq!(loads[0].name, "Hpmcu");
        assert_eq!(loads[0].load_addr, 0x40000);
    }

    #[test]
    fn nontb_ini_has_no_mcu_loads() {
        assert!(parse_ini_mcu_loads(NONTB_INI).is_empty());
    }

    #[test]
    fn parses_reserved_ranges() {
        let r = parse_reserved_ranges(DT_WITH_RTOS);
        assert_eq!(r.len(), 2);
        assert!(r.iter().any(|x| x.start == 0x40000 && x.size == 0x3c000));
        assert!(r.iter().any(|x| x.start == 0x0f00_0000));
    }

    /// The c8a3 brick, prevented: Hpmcu@0x40000 with NO reserving node -> a finding.
    #[test]
    fn flags_the_c8a3_brick() {
        let loads = parse_ini_mcu_loads(TB_INI);
        let reserved = parse_reserved_ranges(DT_NO_RTOS);
        let f = check(&loads, &reserved);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].load.load_addr, 0x40000);
    }

    /// The fix: add the rtos@40000 reservation -> the same config passes.
    #[test]
    fn passes_when_rtos_reserved() {
        let loads = parse_ini_mcu_loads(TB_INI);
        let reserved = parse_reserved_ranges(DT_WITH_RTOS);
        assert!(check(&loads, &reserved).is_empty());
    }

    /// Our board (no boot-loaded MCU) always passes, reserved or not.
    #[test]
    fn nontb_board_always_passes() {
        let loads = parse_ini_mcu_loads(NONTB_INI);
        assert!(check(&loads, &parse_reserved_ranges(DT_NO_RTOS)).is_empty());
    }

    // A reserving node whose LABEL and a sibling property both contain "reg", with
    // an unrelated cell property (`interrupts`) right before the real `reg`.
    const DT_REG_SUBSTRING: &str = r#"
        reserved-memory {
            #address-cells = <1>;
            #size-cells = <1>;
            ranges;
            region-a@40000 { interrupts = <0 43 4>; reg-names = "ctrl"; reg = <0x40000 0x3c000>; no-map; };
        };
    "#;

    /// Regression (correctness review): `reg` must match as a whole property token,
    /// not a bare substring: the `region-*` label, `reg-names`, and the
    /// `interrupts` cells must NOT be misparsed into extra ranges.
    #[test]
    fn reg_token_not_substring() {
        let r = parse_reserved_ranges(DT_REG_SUBSTRING);
        assert_eq!(r.len(), 1, "only the real `reg` property is a range: {r:?}");
        assert_eq!(r[0].start, 0x40000);
        assert_eq!(r[0].size, 0x3c000);
    }

    // A future coprocessor loader named nothing like hpmcu/mcu/amp, at an
    // unreserved DRAM address: the exact case an MCU-name allowlist would miss.
    const UNKNOWN_MCU_INI: &str = r#"
[LOADER_OPTION]
NUM=2
LOADER1=FlashData
LOADER2=Rtos
FlashData=bin/rv11/rv1106_ddr.bin
Rtos=bin/rv11/rv1106_rtos.bin
[LOADER2_PARAM]
LOAD_ADDR=0x40000
"#;

    /// Regression (correctness review): fail closed. A loader that isn't a known-safe
    /// boot component and declares a LOAD_ADDR is treated as a coprocessor load and
    /// checked, even though its name matches no MCU keyword.
    #[test]
    fn unknown_coprocessor_name_is_flagged() {
        let loads = parse_ini_mcu_loads(UNKNOWN_MCU_INI);
        assert_eq!(
            loads.len(),
            1,
            "the unknown loader must be checked: {loads:?}"
        );
        assert_eq!(loads[0].load_addr, 0x40000);
        // and it's caught when no reserved-memory node covers it:
        assert_eq!(check(&loads, &parse_reserved_ranges(DT_NO_RTOS)).len(), 1);
    }

    // Coprocessor names that merely *contain* a safe boot-chain word, each with a
    // LOAD_ADDR: "AudioLoader" contains "loader", "SplRtos" contains "spl", "Bl32" is not "bl31".
    const SUBSTRING_TRAP_INI: &str = r#"
[LOADER_OPTION]
NUM=4
LOADER1=FlashData
LOADER2=AudioLoader
LOADER3=SplRtos
LOADER4=Bl32
FlashData=bin/ddr.bin
AudioLoader=bin/audio.bin
SplRtos=bin/rtos.bin
Bl32=bin/bl32.bin
[LOADER2_PARAM]
LOAD_ADDR=0x50000
[LOADER3_PARAM]
LOAD_ADDR=0x60000
[LOADER4_PARAM]
LOAD_ADDR=0x70000
"#;

    /// Regression (2nd-pass correctness review): the safe allowlist matches a WHOLE
    /// normalized name, never a substring: a coprocessor whose name contains a
    /// boot-chain word must NOT be waved through. FlashData (a real boot component,
    /// no LOAD_ADDR here) stays safe; the three coprocessors are all checked.
    #[test]
    fn safe_loader_is_whole_name_not_substring() {
        let loads = parse_ini_mcu_loads(SUBSTRING_TRAP_INI);
        let mut addrs: Vec<u64> = loads.iter().map(|l| l.load_addr).collect();
        addrs.sort();
        assert_eq!(addrs, vec![0x50000, 0x60000, 0x70000], "loads: {loads:?}");
    }
}
