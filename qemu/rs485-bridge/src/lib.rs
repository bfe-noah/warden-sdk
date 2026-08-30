//! Bridge a QEMU serial chardev (unix socket) to `warden_sim::ModbusSlave`.
//!
//! The guest side is the *master* (flare-edge's `warden-modbus` scanner, polling
//! what it believes is /dev/ttyS4); this bridge is the wire and every slave on
//! it. Frames are delimited by an inter-frame gap of silence: RTU's 3.5-char
//! rule cannot survive a socket transport, so a wall-clock gap stands in for it.
//! A mis-split frame fails CRC inside `handle_frame`, which answers `None` —
//! exactly a real slave staying silent — and the master already treats silence
//! as a timeout, so the failure mode degrades to a dropped poll, never a
//! phantom reply.
//!
//! A second unix socket (the control channel) scripts the simulated bus from
//! test harnesses: fault injection (`drop`, `exception`, `clear`) and register
//! seeding/reading, one command per line.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::time::Duration;
use warden_sim::ModbusSlave;

/// Default inter-frame gap. Generous next to real RTU (3.5 chars at 9600 baud
/// is ~4 ms) because a loaded host can stall a reader; the guest master's
/// response timeout is orders of magnitude larger.
pub const DEFAULT_GAP: Duration = Duration::from_millis(10);

/// The shared bus: the slave plus its declared dimensions. The sim's register
/// setters panic on out-of-range indices (deliberate test-harness semantics);
/// the control channel must bounds-check first so a typo in a scenario script
/// answers `err` instead of killing the VM's whole field bus.
pub struct Bus {
    pub slave: Mutex<ModbusSlave>,
    pub regs: usize,
    pub bits: usize,
}

impl Bus {
    pub fn new(address: u8, regs: usize, bits: usize) -> Self {
        Bus {
            slave: Mutex::new(ModbusSlave::new(address, regs, bits)),
            regs,
            bits,
        }
    }
}

/// Pump one serial connection until EOF: accumulate bytes, dispatch a frame to
/// the slave after `gap` of silence, write back the reply when the slave
/// answers. Any pending bytes are dispatched on EOF so a final unflushed frame
/// is not lost.
pub fn pump_serial(
    stream: &UnixStream,
    slave: &Mutex<ModbusSlave>,
    gap: Duration,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(gap))?;
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        match (&*stream).read(&mut chunk) {
            Ok(0) => {
                if !buf.is_empty() {
                    dispatch(&mut buf, stream, slave)?;
                }
                return Ok(());
            }
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if !buf.is_empty() {
                    dispatch(&mut buf, stream, slave)?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

fn dispatch(
    buf: &mut Vec<u8>,
    stream: &UnixStream,
    slave: &Mutex<ModbusSlave>,
) -> std::io::Result<()> {
    let reply = slave.lock().unwrap().handle_frame(buf);
    match &reply {
        Some(r) => eprintln!("rs485: {} -> {}", hex(buf), hex(r)),
        None => eprintln!("rs485: {} -> (silence)", hex(buf)),
    }
    buf.clear();
    if let Some(r) = reply {
        (&*stream).write_all(&r)?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

/// Execute one control-channel command against the slave. One command per
/// line; the reply is `ok`, `ok <value>`, or `err <reason>`.
///
///   drop <n>            answer the next n requests with silence
///   exception <code>    NAK everything with this exception code (0x01..)
///   clear               clear injected faults
///   holding <addr>=<v>  seed a holding register
///   input <addr>=<v>    seed an input register
///   coil <addr>=<0|1>   seed a coil
///   discrete <addr>=<0|1> seed a discrete input
///   get-holding <addr>  read a holding register back
///   get-coil <addr>     read a coil back
///   ping                liveness check
pub fn handle_control_line(line: &str, bus: &Bus) -> String {
    let mut words = line.split_whitespace();
    let cmd = match words.next() {
        Some(c) => c,
        None => return "err empty command".into(),
    };
    let arg = words.next();
    if words.next().is_some() {
        return format!("err trailing arguments after '{cmd}'");
    }
    let bound = |cmd: &str| match cmd {
        "holding" | "input" | "get-holding" => bus.regs,
        _ => bus.bits,
    };
    let mut s = bus.slave.lock().unwrap();
    match (cmd, arg) {
        ("ping", None) => "ok".into(),
        ("clear", None) => {
            s.clear_faults();
            "ok".into()
        }
        ("drop", Some(n)) => match n.parse::<usize>() {
            Ok(n) => {
                s.drop_next(n);
                "ok".into()
            }
            Err(_) => format!("err bad count '{n}'"),
        },
        ("exception", Some(c)) => match parse_u16(c) {
            Some(c) if c <= 0xff => {
                s.force_exception(c as u8);
                "ok".into()
            }
            _ => format!("err bad exception code '{c}'"),
        },
        ("holding" | "input" | "coil" | "discrete", Some(kv)) => {
            let (addr, val) = match kv.split_once('=') {
                Some((a, v)) => (parse_u16(a), parse_u16(v)),
                None => (None, None),
            };
            match (addr, val) {
                (Some(a), _) if (a as usize) >= bound(cmd) => {
                    format!("err address {a} out of range (0..{})", bound(cmd))
                }
                (Some(a), Some(v)) => {
                    match cmd {
                        "holding" => s.set_holding(a as usize, v),
                        "input" => s.set_input(a as usize, v),
                        "coil" => s.set_coil(a as usize, v != 0),
                        _ => s.set_discrete(a as usize, v != 0),
                    }
                    "ok".into()
                }
                _ => format!("err expected <addr>=<value>, got '{kv}'"),
            }
        }
        ("get-holding" | "get-coil", Some(a)) => match parse_u16(a) {
            Some(a) if (a as usize) >= bound(cmd) => {
                format!("err address {a} out of range (0..{})", bound(cmd))
            }
            Some(a) if cmd == "get-holding" => format!("ok {}", s.holding(a as usize)),
            Some(a) => format!("ok {}", u8::from(s.coil(a as usize))),
            None => format!("err bad address '{a}'"),
        },
        _ => format!("err unknown or malformed command '{line}'"),
    }
}

fn parse_u16(s: &str) -> Option<u16> {
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use warden_sim::modbus::{crc_ok, read_holding};

    // Test gap is larger than DEFAULT_GAP so a loaded CI runner cannot split
    // a frame that the test wrote in two deliberate chunks.
    const GAP: Duration = Duration::from_millis(25);
    const SETTLE: Duration = Duration::from_millis(100);

    fn bus() -> Bus {
        let b = Bus::new(1, 16, 16);
        b.slave.lock().unwrap().set_holding(2, 0xbeef);
        b
    }

    fn with_pump<F: FnOnce(&UnixStream)>(bus: &Bus, f: F) {
        let (master, wire) = UnixStream::pair().unwrap();
        thread::scope(|sc| {
            sc.spawn(|| pump_serial(&wire, &bus.slave, GAP).unwrap());
            f(&master);
            master.shutdown(std::net::Shutdown::Both).unwrap();
        });
    }

    fn read_reply(master: &UnixStream) -> Vec<u8> {
        master.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = [0u8; 256];
        let n = (&*master).read(&mut buf).expect("expected a reply frame");
        buf[..n].to_vec()
    }

    #[test]
    fn whole_frame_gets_a_valid_reply() {
        let s = bus();
        with_pump(&s, |master| {
            (&*master).write_all(&read_holding(1, 2, 1)).unwrap();
            let reply = read_reply(master);
            assert!(crc_ok(&reply), "reply must carry a valid CRC");
            // addr, fc, byte count, 0xbeef
            assert_eq!(&reply[..5], &[1, 0x03, 2, 0xbe, 0xef]);
        });
    }

    #[test]
    fn frame_split_across_writes_within_gap_is_one_frame() {
        let s = bus();
        with_pump(&s, |master| {
            let req = read_holding(1, 2, 1);
            let (a, b) = req.split_at(3);
            (&*master).write_all(a).unwrap();
            thread::sleep(Duration::from_millis(2)); // well inside GAP
            (&*master).write_all(b).unwrap();
            let reply = read_reply(master);
            assert_eq!(&reply[..5], &[1, 0x03, 2, 0xbe, 0xef]);
        });
    }

    #[test]
    fn two_frames_separated_by_gap_get_two_replies() {
        let s = bus();
        with_pump(&s, |master| {
            (&*master).write_all(&read_holding(1, 2, 1)).unwrap();
            let first = read_reply(master);
            assert_eq!(&first[..5], &[1, 0x03, 2, 0xbe, 0xef]);
            thread::sleep(SETTLE);
            (&*master).write_all(&read_holding(1, 2, 1)).unwrap();
            let second = read_reply(master);
            assert_eq!(second, first);
        });
    }

    #[test]
    fn injected_drop_is_silence_then_recovery() {
        let s = bus();
        assert_eq!(handle_control_line("drop 1", &s), "ok");
        with_pump(&s, |master| {
            (&*master).write_all(&read_holding(1, 2, 1)).unwrap();
            master
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let mut buf = [0u8; 16];
            assert!(
                (&*master).read(&mut buf).is_err(),
                "dropped request must produce silence"
            );
            (&*master).write_all(&read_holding(1, 2, 1)).unwrap();
            let reply = read_reply(master);
            assert_eq!(&reply[..5], &[1, 0x03, 2, 0xbe, 0xef]);
        });
    }

    #[test]
    fn control_seeds_and_reads_registers() {
        let s = bus();
        assert_eq!(handle_control_line("holding 5=1234", &s), "ok");
        assert_eq!(handle_control_line("get-holding 5", &s), "ok 1234");
        assert_eq!(handle_control_line("coil 3=1", &s), "ok");
        assert_eq!(handle_control_line("get-coil 3", &s), "ok 1");
        assert_eq!(handle_control_line("holding 0xF=0xff", &s), "ok");
        assert_eq!(handle_control_line("get-holding 15", &s), "ok 255");
        // Out of range must answer err, never panic the bus (16-reg slave).
        assert!(handle_control_line("holding 0x10=0xff", &s).starts_with("err"));
        assert!(handle_control_line("get-holding 16", &s).starts_with("err"));
        assert!(handle_control_line("coil 16=1", &s).starts_with("err"));
        assert_eq!(handle_control_line("ping", &s), "ok");
    }

    #[test]
    fn control_rejects_malformed_lines() {
        let s = bus();
        assert!(handle_control_line("", &s).starts_with("err"));
        assert!(handle_control_line("drop many", &s).starts_with("err"));
        assert!(handle_control_line("holding 5", &s).starts_with("err"));
        assert!(handle_control_line("exception 300", &s).starts_with("err"));
        assert!(handle_control_line("frobnicate 1", &s).starts_with("err"));
        assert!(handle_control_line("drop 1 2", &s).starts_with("err"));
    }

    #[test]
    fn forced_exception_naks_and_clear_recovers() {
        let s = bus();
        assert_eq!(handle_control_line("exception 0x02", &s), "ok");
        with_pump(&s, |master| {
            (&*master).write_all(&read_holding(1, 2, 1)).unwrap();
            let nak = read_reply(master);
            assert_eq!(&nak[..3], &[1, 0x83, 0x02], "fc|0x80 + exception code");
            assert_eq!(handle_control_line("clear", &s), "ok");
            thread::sleep(SETTLE);
            (&*master).write_all(&read_holding(1, 2, 1)).unwrap();
            let reply = read_reply(master);
            assert_eq!(&reply[..5], &[1, 0x03, 2, 0xbe, 0xef]);
        });
    }
}
