# drivers/ — our own hardened, hardware-facing drivers

Per **ADR-0002** (tiered MC/DC) and **ADR-0005** (source-of-truth), our own
hardware-facing code migrates here behind a HAL seam and is hardened. "100% MC/DC on
100% of drivers" is infeasible (≈97% of kernel-driver LOC is vendor blobs — AIC8800
alone is 88.5K lines); the realistic, honest target is tiered.

## Tier 1 — real 100% MC/DC (here now, CI-enforced)

Self-contained logic with a clean seam, measured to **100% MC/DC** (gcc-14
`-fcondition-coverage`) by the CI `mcdc` job (`make -C drivers/*/test check`):

| Driver | Seam | MC/DC |
|---|---|---|
| `relays/` | `relay_io` vtable (sysfs backend + in-memory fake) | 40/40 conditions, 100% |
| `freshness/` | produce/render callbacks (the "no stale numbers" guard) | 66/66 conditions, 100% |

**Adding a Tier-1 driver:** copy `<name>.{c,h}` here, put the hardware/OS calls behind
a small injectable seam, then mirror `relays/test/` (a fake backend for the logic
branches + a real backend over a scratch tree for the plumbing) and
`enforce-mcdc.sh`. The CI job picks up any `drivers/*/test/Makefile` automatically.

## Tier 2 — serious testing + fault-injection + benchmarks

Drivers too large or too vendor/UI-coupled for literal MC/DC get fault-injection,
branch coverage, and benchmarks against the simulator instead. Their **hardware side
is already modelled and tested here** in `../sim/`:

| Driver | Serious-testing status | SDK model |
|---|---|---|
| `modbus_engine.c` (RS485 master) | 11 pty scenarios + fault-injection + a compiled corpus walk (flare-edge `tools/modbus-sim/`, green) | `sim::modbus` RTU slave (11 tests, silent-drop/forced-NAK faults) + `modbus_read_holding` benchmark |
| `warden_rga.c` (RGA offload) | offload-dispatch + CPU-fallback logic | `sim::rga` recording `improcess` fake (programmable IM_STATUS) + `rga_improcess` benchmark |
| HPMCU supervisor (`hpmcu.rs`) | arm/beat/fire + boot-grace safety property | `sim::hpmcu` (8 tests) + `hpmcu_tick` benchmark |

**Why the Tier-2 *source* isn't vendored here yet:** `modbus_engine.c` and
`warden_rga.c` pull in shared UI headers (`platform.h`, `settings.h`, `lv_*`) and
librga. Copying those in would duplicate exactly the shared surface the
**flare-edge↔warden-sdk unification** (ADR-0003/0005, a separate Noah-gated step) is
meant to resolve cleanly. So the Tier-2 *models* (the hardware ends) live here now;
the Tier-2 *driver sources* migrate in with the unification, at which point their
existing flare-edge harnesses point at this repo.
