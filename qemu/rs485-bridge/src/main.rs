//! CLI wiring for the RS-485 bridge. All behavior lives in the lib (tested
//! there); this file only parses arguments, connects sockets, and spawns the
//! control listener.
//!
//! Typical use (matches qemu/run.sh --rs485). Put the sockets in a private
//! per-run directory (mktemp -d): short (AF_UNIX caps paths at ~108 chars)
//! and not guessable/pre-creatable by other local users, unlike a fixed
//! /tmp name:
//!
//!   d=$(mktemp -d /tmp/rs485.XXXXXX)
//!   qemu/run.sh --kernel ... --rs485 "$d/serial.sock" &
//!   rs485-bridge --serial "$d/serial.sock" --control "$d/ctl.sock"

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::exit;
use std::time::{Duration, Instant};
use warden_rs485_bridge::{handle_control_line, pump_serial, Bus, DEFAULT_GAP};

fn usage() -> ! {
    eprintln!(
        "usage: rs485-bridge --serial <sock> [--control <sock>] [--address N] \
         [--regs N] [--bits N] [--gap-ms N]"
    );
    exit(2);
}

fn main() {
    let mut serial: Option<String> = None;
    let mut control: Option<String> = None;
    let mut address: u8 = 1;
    let mut regs: usize = 128;
    let mut bits: usize = 64;
    let mut gap = DEFAULT_GAP;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = |name: &str| {
            let v = args.next().unwrap_or_else(|| {
                eprintln!("{name} needs a value");
                usage()
            });
            // A following flag means the value was omitted. Report the real
            // problem instead of swallowing the flag as a bogus value.
            if v.starts_with("--") {
                eprintln!("{name} needs a value, got flag '{v}'");
                usage()
            }
            v
        };
        match a.as_str() {
            "--serial" => serial = Some(val("--serial")),
            "--control" => control = Some(val("--control")),
            "--address" => address = val("--address").parse().unwrap_or_else(|_| usage()),
            "--regs" => regs = val("--regs").parse().unwrap_or_else(|_| usage()),
            "--bits" => bits = val("--bits").parse().unwrap_or_else(|_| usage()),
            "--gap-ms" => {
                gap = Duration::from_millis(val("--gap-ms").parse().unwrap_or_else(|_| usage()))
            }
            _ => usage(),
        }
    }
    let serial = serial.unwrap_or_else(|| usage());

    // The bus is shared between the serial pump and the control channel.
    // 'static so the control thread needs no scoped lifetime: the bridge runs
    // until killed.
    let bus: &'static Bus = Box::leak(Box::new(Bus::new(address, regs, bits)));

    if let Some(path) = control {
        // Clear a stale socket from a previous run. A failure here that is not
        // "nothing to remove" (e.g. someone else's file behind /tmp's sticky
        // bit) will make the bind below fail. Surface both errors.
        let removed = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap_or_else(|e| {
            eprintln!("FATAL: cannot bind control socket {path}: {e}");
            if let Err(re) = removed {
                if re.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("       (removing the pre-existing file also failed: {re})");
                }
            }
            exit(1);
        });
        eprintln!("rs485: control socket at {path}");
        std::thread::spawn(move || {
            // Explicit error handling: `.flatten()` would turn a persistent
            // accept() failure (fd exhaustion etc.) into a silent hot loop.
            for conn in listener.incoming() {
                let conn = match conn {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("rs485: control accept failed: {e}, backing off");
                        std::thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                };
                let reader = BufReader::new(conn.try_clone().expect("clone control conn"));
                let mut writer = conn;
                for line in reader.lines() {
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    let reply = handle_control_line(&line, bus);
                    if writeln!(writer, "{reply}").is_err() {
                        break;
                    }
                }
            }
        });
    }

    // QEMU (chardev server=on) may come up after us: retry the connect briefly
    // instead of racing the VM launch.
    let deadline = Instant::now() + Duration::from_secs(15);
    let stream = loop {
        match UnixStream::connect(&serial) {
            Ok(s) => break s,
            Err(e) if Instant::now() < deadline => {
                eprintln!("rs485: waiting for {serial} ({e})");
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(e) => {
                eprintln!("FATAL: cannot connect serial socket {serial}: {e}");
                exit(1);
            }
        }
    };
    eprintln!("rs485: connected to {serial}, slave address {address}, gap {gap:?}");

    if let Err(e) = pump_serial(&stream, &bus.slave, gap) {
        eprintln!("FATAL: serial pump: {e}");
        exit(1);
    }
    eprintln!("rs485: serial closed (VM gone), exiting");
}
