//! config-lint CLI: the MCU-load-vs-reserved-memory gate as a CI check.
//!
//! Usage:
//!     config-lint --ini <loader.ini> --dt <devicetree.dts>
//!
//! The `--dt` argument accepts a `.dts`/`.dtsi` or the flattened output of
//! `dtc -I dtb -O dts built.dtb` (preferred in CI: it resolves includes and
//! overlays, so it reflects what the board will actually boot). Exits non-zero
//! and prints each offending MCU load if any lands outside a `reserved-memory`
//! node: the 0x40000 brick class.

use std::process::ExitCode;
use warden_config_lint::{check, parse_ini_mcu_loads, parse_reserved_ranges};

fn main() -> ExitCode {
    let mut ini_path = None;
    let mut dt_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--ini" => ini_path = args.next(),
            "--dt" => dt_path = args.next(),
            "-h" | "--help" => {
                eprintln!("usage: config-lint --ini <loader.ini> --dt <devicetree.dts>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("config-lint: unknown argument {other:?}");
                return ExitCode::from(2);
            }
        }
    }

    let (Some(ini_path), Some(dt_path)) = (ini_path, dt_path) else {
        eprintln!("usage: config-lint --ini <loader.ini> --dt <devicetree.dts>");
        return ExitCode::from(2);
    };

    let ini = match std::fs::read_to_string(&ini_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("config-lint: cannot read {ini_path}: {e}");
            return ExitCode::from(2);
        }
    };
    let dt = match std::fs::read_to_string(&dt_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("config-lint: cannot read {dt_path}: {e}");
            return ExitCode::from(2);
        }
    };

    let loads = parse_ini_mcu_loads(&ini);
    let reserved = parse_reserved_ranges(&dt);
    let findings = check(&loads, &reserved);

    if loads.is_empty() {
        println!("config-lint: no boot-loaded MCU firmware in {ini_path}: nothing to reserve.");
    } else {
        println!(
            "config-lint: {} MCU load(s) in {ini_path}, {} reserved-memory range(s) in {dt_path}.",
            loads.len(),
            reserved.len()
        );
    }

    if findings.is_empty() {
        println!("config-lint: OK: every MCU load is inside a reserved-memory node.");
        ExitCode::SUCCESS
    } else {
        for f in &findings {
            eprintln!("config-lint: FAIL: {}", f.msg);
        }
        ExitCode::FAILURE
    }
}
