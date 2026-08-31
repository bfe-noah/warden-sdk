# ADR 0002: Tiered MC/DC

**Status:** Accepted (2026-08-25).

## Context
The goal "port + harden 100% of drivers to 100% MC/DC" is infeasible as literally
stated: ~97% of driver LOC is vendor blobs (the AIC8800 wifi driver alone is 88.5K
lines) that we cannot meaningfully unit-test to MC/DC on the host. Forcing MC/DC on
that code would be theatre, not assurance.

## Decision
A **two-tier** policy, measured against the Hardware Abstraction Seam:

- **Tier 1, our own hardware-facing code -> real 100% MC/DC.** `modbus_engine.c`,
  `relays.c`, `warden_rga.c` (wrapper), `hpmcu.rs`, `devmem.rs`, `freshness.c`, plus
  the two smallest near-mainline drivers where feasible. Enforced in CI
  (`gcc-14 -fcondition-coverage` + `gcov-14 --conditions`). Rust uses
  `cargo-llvm-cov` line/region coverage for now; true `--mcdc` needs a nightly
  toolchain (`-Z coverage-options=condition`) and is deferred on that tooling skew;
  the C drivers carry the literal MC/DC gate.
- **Tier 2, ported/vendor drivers -> fault-injection + branch coverage + benchmarks**
  against the simulator, behind a stable seam. Explicitly NOT literal MC/DC.

## Consequences
- Matches the user's framing: "as many drivers as possible at 100% MC/DC; for the
  rest, a very serious testing and benchmarking system."
- The seam is the shared object: the same thing MC/DC is measured against and the
  simulator implements; the two goals reinforce, not duplicate.
- Every Tier-1 file gets a `drivers/<name>/test/` host harness (a `Makefile` +
  `test_<name>.c`) that calls the one shared `drivers/enforce-mcdc.sh`, which derives
  the driver name from the `.gcov` file, so there is a single gate to maintain, not a
  per-driver copy. The CI `mcdc` job auto-discovers any `drivers/*/test/Makefile` and
  fails below 100%.
