// Differential clock probe for warden-sdk issue #3: compares the musl/vDSO
// CLOCK_MONOTONIC rate against the kernel's own /proc/uptime over a fixed
// interval, so boot-time offsets cancel and only the RATE ratio remains.
use std::{fs, thread, time::Duration};

fn mono(clock: libc::clockid_t) -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(clock, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

fn uptime() -> f64 {
    fs::read_to_string("/proc/uptime").unwrap()
        .split_whitespace().next().unwrap().parse().unwrap()
}

fn main() {
    let interval = 10.0;
    let (m0, r0, u0) = (mono(libc::CLOCK_MONOTONIC), mono(libc::CLOCK_MONOTONIC_RAW), uptime());
    thread::sleep(Duration::from_secs_f64(interval));
    let (m1, r1, u1) = (mono(libc::CLOCK_MONOTONIC), mono(libc::CLOCK_MONOTONIC_RAW), uptime());
    let (dm, dr, du) = (m1 - m0, r1 - r0, u1 - u0);
    println!("CLOCKPROBE monotonic={dm:.4} raw={dr:.4} uptime={du:.4} ratio_mono={:.5} ratio_raw={:.5}", dm / du, dr / du);
    println!("CLOCKPROBE abs monotonic={m1:.2} uptime={u1:.2} abs_ratio={:.5}", m1 / u1);
}
