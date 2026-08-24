# warden-sdk architecture

How the SDK makes WardenOS buildable, testable, and hardenable without a panel in
the loop. Grounded in a full survey of the current flare-edge firmware (the seam
inventory below is from that survey, not aspiration).

## 1. The problem the seams solve

The firmware touches RV1106 hardware through a *grab-bag* of mechanisms, each
tested (or not) differently. Today:

| Block | Where | Access | Test seam today | Fails on host by |
|---|---|---|---|---|
| Registers / SRAM (CRU reset, HPMCU mailbox) | `flared/src/devmem.rs`, `hpmcu.rs` | `/dev/mem` mmap `peek/poke32` | **none** — zero tests | (would fault; not exercised) |
| HPMCU / RISC-V coproc | `flared/src/hpmcu.rs` | via devmem + firmware blob load | `WARDEN_HPMCU_FW` redirects the blob path only | env gate disables it |
| NPU load | `ui-src/.../sysmon.c` | read `/proc/rknpu/load` | **none** — literal path | file absent → "NPU absent" |
| RGA (2D blit) | `ui-src/.../warden_rga.c` | `librga improcess` + dma-heap ioctl | compile-time `#if WARDEN_USE_RGA` | `#if` off → LVGL software path |
| RS485 daemon | `warden-modbus/modbus_engine.c` | `open("/dev/ttyS4")` | recompile `-DRS485_PORT=<pty>` | (recompiled for a pty) |
| RS485 panel client | `ui-src/.../modbus.c` | `AF_UNIX` socket | `WARDEN_MODBUS_SOCK` env override | socket absent → "unavailable" |
| Relays / GPIO | `ui-src/.../relays.c` | `/sys/class/gpio` sysfs | **none** — literal paths | path absent → "unavailable" |
| Slot metadata | `flared/src/slotctl.rs` | `misc` partition + `/proc/cmdline` | `WARDEN_MISC_DEV`, `WARDEN_CMDLINE_FILE` env overrides | (redirected to scratch files) |

Three patterns coexist: **compile-time `#if`** (RGA), **env-override** (modbus
socket, misc dev, cmdline, hpmcu fw), and **fails-soft-because-the-path-is-absent**
(NPU, relays, devmem-would-fault). The last is not a test seam — you cannot inject
"relay 1 is ON" or "NPU at 80%", only "absent". The SDK's job is to turn all of
these into **one deliberate seam per block** with a real backend and a sim backend.

## 2. The seam taxonomy

Two seam kinds cover everything above:

- **Register/SRAM seam → a trait.** `MemBus` (`sim/src/membus.rs`): `peek32/poke32`
  at a physical address. Real backend = flared `devmem.rs` mmap; sim backend =
  `SimBus` (in-memory word map, `Clone` so two "cores" alias shared memory). The
  HPMCU watchdog and the CRU reset ladder both ride this. **Built.**
- **Resource-path seam → env-override + injection.** For file/socket/sysfs paths
  (`/proc/rknpu/load`, `/sys/class/gpio/*`, `/dev/ttyS4`, `misc`), generalize the
  existing `WARDEN_MISC_DEV`/`WARDEN_MODBUS_SOCK` pattern into one rule: **every
  device/proc/sys path a driver opens is resolved through a single indirection
  (`warden_hw_path("npu.load")` in C, an env-overridable const in Rust)**, so a
  test points it at a fake file/fifo the sim writes. No LD_PRELOAD, no fake mounts.

RGA stays compile-time — its `#if WARDEN_USE_RGA` already cleanly isolates the
librga/dma-heap calls behind the always-compiled LVGL draw-unit glue; the sim
backend is "a fake `improcess` that records the blits it was asked to do", swapped
behind the same `#if`, so the offload *dispatch* logic gets tested even though the
blit itself is modelled.

## 3. The simulator (`sim/`)

A host Rust library modelling the hardware the vendor SDK cannot, so driver and
supervisor logic runs in CI with no panel.

- **`membus` — register/SRAM bus.** Done. `MemBus` trait + `SimBus`.
- **`hpmcu` — the RISC-V watchdog coprocessor.** Done. Faithful port of
  `hpmcu/watchdog/main.c`'s state machine (boot-grace, heartbeat-timeout, disarm,
  fire) against a `SimBus` mailbox, virtual clock, 8 tests including the
  arm-within-grace no-boot-loop safety property. This is the model that would have
  let the boot-loaded-watchdog logic be validated before the flash that bricked a
  bench unit (though the *layout* fault — a load address in unreserved kernel RAM —
  is a target-config check, §5, not a sim property).
- **`cru` — reset ladder.** Done. `CruSim` on `MemBus` (so `flared::devmem::hard_reset`'s
  ladder is host-tested against the known glb_srst_fst / DW-watchdog registers); an
  **NPU** load model behind the path seam; a **GPIO/relay** sysfs model; a **modbus
  device** model unifying the existing `mbsim.py` corpus into the same framework;
  an **RGA** recording fake.

Integration with flare-edge: flared implements `MemBus` for `/dev/mem` and gains
`#[cfg(test)]` tests driving its real arm/beat logic against `HpmcuSim`. This needs
warden-sdk reachable as a Cargo dependency in CI — i.e. a remote for this repo,
which is a **[maintainer]-go-ahead item** (credential/remote creation). Until then the
firmware-side seam and a local test double land in flare-edge, unified with `sim/`
once the dependency exists. No duplication of *logic* — only the tiny trait.

## 4. Driver hardening (the "port + harden to MC/DC" goal)

"100% MC/DC on 100% of drivers" is infeasible literally: ~97% of driver LOC is
vendor blobs (AIC8800 wifi = 88.5K lines). Tiered target:

- **Tier 1 — our own hardware code → real MC/DC.** modbus master (`modbus_engine.c`),
  relays (`relays.c`), the RGA wrapper's dispatch, the HPMCU supervisor
  (`hpmcu.rs`), the devmem reset ladder. Method: the proven `tests/uboot-ab`
  pattern — extract the unit, mock its world, build `-fcondition-coverage`, enforce
  with `enforce-mcdc.sh` (gcc-14 `gcov --conditions`). **Gap the survey found: there
  is no C-side coverage in CI at all today** — only flared line-coverage and the one
  uboot-ab MC/DC file. Standing up an MC/DC harness for the first C driver
  (`relays.c` — small, safety-relevant) is the first driver-hardening deliverable.
- **Tier 2 — near-mainline small drivers → branch coverage + fault injection.**
- **Tier 3 — vendor blobs (AIC8800, MPP/ISP/RGA libs) → fault-injection hardening
  behind the seam,** not MC/DC. The AIC8800 SDIO-wedge Tier-1 fix + the designed
  reset-on-ETIMEDOUT recovery are this tier: test the *recovery* path against an
  injected wedge on the `MemBus`/SDIO seam, since the blob itself is untestable.

Every seam gets a fault-injection mode (a wedged SDIO link, a stalled MCU
heartbeat, an RGA timeout, a GPIO write EIO) so recovery code is tested against
failure, not just the happy path.

## 5. Target-config checks (a class the sim cannot cover)

The brick was a *memory-map* fault: the boot-loaded MCU's load address (`0x40000`)
is a reserved carve-out on Thunder-Boot boards but plain kernel RAM on ours. No
behavioural sim catches that — it needs a **static check against the target DT**:
"every address the MCU/coprocessor code loads to is inside a `reserved-memory`
node." warden-sdk owns these config-lint checks (idblock loader `.ini` vs DT
reservations, partition table vs image sizes, vermagic vs kernel) as CI gates, so a
mistake is caught before a flash rather than on the bench.

**Built:** `tools/config-lint` implements the first and most important of these —
the MCU-load-vs-`reserved-memory` gate. It parses the rkbin loader `.ini` for
every `LOADERn=Hpmcu` firmware and its `[LOADERn_PARAM] LOAD_ADDR`, parses the
target devicetree (`.dts`, or `dtc -I dtb` output in CI) for `reserved-memory`
ranges, and fails if any MCU load lands outside a reservation. Its test suite
encodes the c8a3 brick itself: the real Thunder-Boot `.ini` (Hpmcu @ `0x40000`)
fails against a DT with no `rtos@40000` node and passes once the reservation is
added. **Next** target-config checks: partition-table-vs-image-size and
vermagic-vs-kernel.

## 6. Kernel forward-port (separate, bounded phase)

Move to **plan44's OpenWrt RV1106 fork — Linux 6.6** (152 RV1106 patches + our
exact board DT), not mainline (no DT/clk/display/RGA/NPU/flash-boot merged). It is
a diff-and-borrow forward-port onto our Buildroot/uClibc base, not a swap-in
(plan44 drops Buildroot for OpenWrt/musl and ships no AIC8800 kmod). The dominant
risk is the struct-ABI break (the VLAN saga) — mitigated by shipping any kernel
move as one matched boot+oem image, never a partial reflash. This phase starts
after the sim + driver-hardening foundation, since those are how we'll regression
the port.

## 7. Order of work

1. **Simulator core** — `membus` + `hpmcu` (done); reset-ladder + path-seam
   scaffolding next.
2. **First C-driver MC/DC harness** — `relays.c`, establishing the C coverage gate.
3. **flared devmem/hpmcu seam + tests** (firmware-side trait; unify with `sim/`
   when the repo has a remote).
4. **Config-lint CI gates** (§5) — the brick-class of bug.
5. **Hermetic image build** wrapper moves in.
6. **Kernel 6.6 forward-port** (§6).
