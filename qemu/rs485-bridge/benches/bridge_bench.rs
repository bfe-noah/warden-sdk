//! Micro-benchmarks for the RS-485 bridge dispatch path: same dependency-free
//! fixed-iteration pattern as sim/benches/sim_bench.rs: human timings to
//! stdout, one JSON line per benchmark to stderr for CI trend capture.
//!
//! Run: `cargo bench` (or `cargo run --release --bench bridge_bench`).

use std::time::Instant;
use warden_rs485_bridge::{handle_control_line, Bus};
use warden_sim::modbus::read_holding;

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

    // Full request->reply dispatch through the locked slave (the per-poll cost
    // a guest master pays on the simulated bus, minus socket I/O).
    {
        let bus = Bus::new(1, 128, 64);
        let req = read_holding(1, 2, 4);
        bench("bridge_dispatch", N, || {
            let _ = bus.slave.lock().unwrap().handle_frame(&req);
        });
    }

    // Control-channel command parse + register write.
    {
        let bus = Bus::new(1, 128, 64);
        bench("control_line", N, || {
            let _ = handle_control_line("holding 5=1234", &bus);
        });
    }
}
