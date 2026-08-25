//! Micro-benchmarks for the warden-sim hardware models.
//!
//! Dependency-free (`harness = false`): a fixed-iteration timing loop, so the sim
//! crate keeps zero deps and CI can capture a stable ns/op number per model with no
//! criterion tree to compile. Human-readable timings go to stdout; one JSON line per
//! benchmark goes to stderr for CI trend capture (`bench: … ns_per_op …`).
//!
//! Run: `cargo bench` (or `cargo run --release --bench sim_bench`).

use std::time::Instant;
use warden_sim::{CruSim, HpmcuSim, MemBus, ModbusSlave, Rect, RgaSim, SimBus, Surface};

fn bench<F: FnMut()>(name: &str, iters: u64, mut f: F) {
    for _ in 0..(iters / 10).max(1) {
        f(); // warm up
    }
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("{name:<24} {ns:>9.1} ns/op   ({iters} iters)");
    eprintln!("{{\"bench\":\"{name}\",\"ns_per_op\":{ns:.1},\"iters\":{iters}}}");
}

fn main() {
    const N: u64 = 1_000_000;

    // HPMCU watchdog: one tick (mailbox poke + two peeks + state machine).
    {
        let mut h = HpmcuSim::new(SimBus::new(), 0, 0);
        let mut now = 0u64;
        bench("hpmcu_tick", N, || {
            now += 1;
            h.tick(now);
        });
    }

    // CRU reset ladder: one poll.
    {
        let mut c = CruSim::new(SimBus::new());
        let mut now = 0u64;
        bench("cru_poll", N, || {
            now += 1;
            let _ = c.poll(now);
        });
    }

    // Modbus RTU: a read-holding-registers request -> response round trip.
    {
        let mut s = ModbusSlave::new(1, 16, 16);
        s.set_holding(0, 0x1234);
        let req = warden_sim::modbus::request(1, &[0x03, 0x00, 0x00, 0x00, 0x04]);
        bench("modbus_read_holding", N, || {
            let _ = s.handle_frame(&req);
        });
    }

    // RGA: one improcess dispatch record (+ clear, so the log stays bounded).
    {
        let mut r = RgaSim::new();
        let sf = Surface {
            width: 720,
            height: 720,
            format: 0,
        };
        let rc = Rect {
            x: 0,
            y: 0,
            w: 720,
            h: 720,
        };
        bench("rga_improcess", N, || {
            let _ = r.improcess(sf, sf, rc, rc);
            r.clear();
        });
    }

    // MemBus: a poke + peek round trip (the /dev/mem seam's hot path).
    {
        let bus = SimBus::new();
        bench("membus_poke_peek", N, || {
            bus.poke32(0x100, 0xdead_beef);
            let _ = bus.peek32(0x100);
        });
    }
}
