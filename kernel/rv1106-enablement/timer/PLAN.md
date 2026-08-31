# arch-timer / vDSO clock fix — plan (issue #3)

Status: DIAGNOSED off-board, fix gated on two bench measurements.

## Established (2026-08-30, qemu/ device sim)

The same kernel family under `qemu-system-arm -M virt` gives musl
`CLOCK_MONOTONIC`/`_RAW` rate ratio 0.99963 vs `/proc/uptime` (interval
measurement, `qemu/tests/clockprobe`). The generic 6.18 armv7 vDSO is
therefore CORRECT; the board symptom (musl reads ~12% high, kernel time
right) is RV1106-specific. The boot chain runs in the secure world and is
closed rkbin — the NS view of the CPU timer registers (CNTFRQ, CNTVOFF) is
whatever it left behind, and only the arch-counter path (vDSO,
`arch_sys_counter`) trusts them.

## What the bench must answer (single boot of the `_b` slot)

Run `clockprobe` (interval mode) on the 6.18 slot:

1. `ratio_mono` far from 1.0 -> RATE error: CNTFRQ wrong. The true rate =
   claimed rate (dmesg `arch_timer: cp15 timer running at X MHz`) times the
   measured ratio.
2. `ratio_mono` ~= 1.0 but `abs_ratio` far from 1.0 -> OFFSET error: CNTVOFF
   left nonzero; rate fine.

(The original issue measured only one absolute sample, which cannot
distinguish these.)

## The fix (both cases, one DT override)

Append to the BOARD dts (`rv1106-warden.dts` — never the vendor dtsi) an
override on the armv7-timer node:

    arm,cpu-registers-not-fw-configured;
    clock-frequency = <MEASURED_HZ>;

The property makes the driver use the physical counter, ignore CNTVOFF, and
take the frequency from DT — the documented remedy for firmware that does
not configure the CPU timer registers. `MEASURED_HZ` comes from bench
answer 1 (do NOT guess; a wrong value makes every clock wrong instead of
one path). Ship as an update to the arch/dts patch in `patches/`.

## Regression guards

- Off-board: `qemu/tests/clock-sanity.sh` asserts the vDSO rate in the VM
  (guards the generic path; cannot see board registers).
- On-board: re-run `clockprobe` on the patched `_b` slot; both ratios and
  the absolute ratio must be ~1.0. Record the numbers here and in issue #3.

## Bench access note

2026-08-30: c8a3 is physically dark (CP2102 console silent through two
remote power cycles; both network paths down) — needs hands at the bench
before the measurements can run.
