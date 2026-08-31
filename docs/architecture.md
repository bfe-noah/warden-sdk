# Architecture

How the SDK makes the 86 Panel buildable, testable, and hardenable without a
board in the loop. The seam inventory below comes from a full survey of the
downstream WardenOS firmware — the SDK's first consumer — not aspiration; the
file paths in it point into that (private) codebase and serve as engineering
context.

## 1. The Problem

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

## 2. Seam Taxonomy

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

## 3. The Register Simulator

A host Rust library modelling the hardware the vendor SDK cannot, so driver and
supervisor logic runs in CI with no panel.

- **`membus` — register/SRAM bus.** Done. `MemBus` trait + `SimBus`.
- **`hpmcu` — the RISC-V watchdog coprocessor.** Done. Faithful port of
  `hpmcu/watchdog/main.c`'s state machine (boot-grace, heartbeat-timeout, disarm,
  fire) against a `SimBus` mailbox, virtual clock, 7 tests including the
  arm-within-grace no-boot-loop safety property. This is the model that would have
  let the boot-loaded-watchdog logic be validated before the flash that bricked a
  bench unit (though the *layout* fault — a load address in unreserved kernel RAM —
  is a target-config check, §5, not a sim property).
- **`cru` — reset ladder.** Done. `CruSim` on `MemBus` (so `flared::devmem::hard_reset`'s
  ladder is host-tested against the known glb_srst_fst / DW-watchdog registers), plus
  the boot-mode register's survives-warm-reset / cleared-by-POR behaviour (the MaskRom
  recovery maneuver). The matching firmware-side `Bus` seam on flared's `devmem` — so
  the shipped ladder can be asserted to poke the confirmed offset, never the wrong-SoC
  one — lands when flare-edge consumes warden-sdk (§8 item 3, maintainer-gated), not yet on
  flare-edge `main`.
- **`modbus` — RS-485 device end.** Done. `ModbusSlave`: a byte-in/byte-out RTU slave
  (CRC16 byte-identical to the master, FC 0x01–0x06/0x0F/0x10/0x11, exception replies,
  and fault injection — silent-drop and forced-NAK) so `warden-modbus`'s master can be
  hardened to MC/DC against realistic device behaviour with no serial hardware. MEI
  (0x2B/0x0E) identification is the documented follow-up. The same slave also serves
  as the QEMU device sim's field bus: `qemu/rs485-bridge/` feeds it from a serial
  chardev so the guest's real master polls it over what it believes is /dev/ttyS4 (§7).
- **`npu` — NPU load model.** Done. `NpuSim` models `/proc/rknpu/load` (the exact
  "NPU load:  N%" text the sysmon reads) behind the path seam, so the load-readout UI
  is host-testable. NPU *compute* is explicitly out of scope — no inference runs here.
- **`rga` — 2D blitter offload.** Done. `RgaSim`, a recording `improcess` fake with a
  programmable `IM_STATUS`, so the RGA offload-dispatch and CPU-fallback logic is
  exercised behind the `#if WARDEN_USE_RGA` seam without librga; wired into the
  `rga_improcess` benchmark.
- **Next:** MEI (0x2B/0x0E) Modbus identification; the Tier-2 driver *sources*
  (`modbus_engine.c`, `warden_rga.c`) migrate in with the flare-edge unification
  (ADR-0005) — their hardware ends are already modelled and tested above.

Integration with flare-edge: flared implements `MemBus` for `/dev/mem` and gains
`#[cfg(test)]` tests driving its real arm/beat logic against `HpmcuSim`. This needs
warden-sdk reachable as a Cargo dependency in CI — i.e. a remote for this repo,
which is a **maintainer go-ahead item** (credential/remote creation). Until then the
firmware-side seam and a local test double land in flare-edge, unified with `sim/`
once the dependency exists. No duplication of *logic* — only the tiny trait.

## 4. Driver Hardening

"100% MC/DC on 100% of drivers" is infeasible literally: ~97% of driver LOC is
vendor blobs (AIC8800 wifi = 88.5K lines). Tiered target:

- **Tier 1 — our own hardware code → real MC/DC.** Method: the proven flare-edge `tests/uboot-ab`
  pattern — extract the unit behind a small injectable seam, mock its world, build
  `-fcondition-coverage`, enforce with the shared `drivers/enforce-mcdc.sh` (gcc-14
  `gcov --conditions`) in the CI `mcdc` job. **Done here now:** `relays.c` (40/40
  conditions) and `freshness.c` (66/66), both at 100% MC/DC and CI-enforced. **Migrate
  in next:** the modbus master (`modbus_engine.c`) and the RGA wrapper's dispatch —
  their hardware ends are already modelled and tested in `sim/` (`modbus`, `rga`); the
  driver sources move in with the flare-edge unification (ADR-0005). The HPMCU
  supervisor and devmem reset ladder are covered Rust-side (`sim/hpmcu`, `sim/cru`).
- **Tier 2 — near-mainline small drivers → branch coverage + fault injection.**
- **Tier 3 — vendor blobs (AIC8800, MPP/ISP/RGA libs) → fault-injection hardening
  behind the seam,** not MC/DC. The AIC8800 SDIO-wedge Tier-1 fix + the designed
  reset-on-ETIMEDOUT recovery are this tier: test the *recovery* path against an
  injected wedge on the `MemBus`/SDIO seam, since the blob itself is untestable.

Every seam gets a fault-injection mode (a wedged SDIO link, a stalled MCU
heartbeat, an RGA timeout, a GPIO write EIO) so recovery code is tested against
failure, not just the happy path.

## 5. Target-Config Checks

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

## 6. Kernel Forward-Port

A self-built **Linux 6.18.46**, forward-ported directly from the vendor 5.10.160 tree
onto our Buildroot LTS/uClibc base — **not** the plan44/OpenWrt 6.6 fork this section
originally reached for. **ADR-0001 records why that was superseded:** plan44 drops
Buildroot for OpenWrt/musl and ships no AIC8800 kmod, so it was a swap-out, not a
forward-port. Mainline alone was not viable either (no DT/clk/display/RGA/NPU/
flash-boot upstream for RV1106); the port reuses the already-in-mainline rv1126
register data where it matches and carries our deltas as the reviewable `patches/`
series. This is **done and hardware-verified on `warden-c8a3`** — clk, pinctrl, eMMC,
GMAC, TRNG, OTP, SARADC/TSADC, RTC, USB host, PWM/backlight, VOP display, GT911 touch,
AIC8800 wifi, RGA, I2S audio, HPMCU mailbox, the open NPU driver, and PVTM all boot.
The dominant risk was the struct-ABI break (the VLAN saga), mitigated by shipping the
kernel move as one matched boot+oem image, never a partial reflash.
`build/build-kernel.sh` (the hermetic build) and the `patches-apply` CI gate keep the
series honest against pristine 6.18.46; provenance is in `patches/README.md` and
`kernel/rv1106-enablement/`.

## 7. Device Emulation

The third simulator, deliberately not named "sim": a QEMU VM (`-M virt,highmem=off`,
one Cortex-A7, 256M — the RV1106G3's shape) that boots the real forward-ported
kernel and real userspace, entering at `-kernel zImage` because everything below
(BootROM, idblock/DDR-init, U-Boot, the BCB A/B machinery) is closed blobs plus
mask ROM. The canonical RV1106 zImage boots virt unmodified; an additive kconfig
fragment (`qemu/configs/virt.fragment` via `WARDEN_KCONFIG_FRAGMENT`) adds the
scenario devices (PCI serial for RS485, i6300esb watchdog, WireGuard,
virtio-gpu/input for the 720x720 UI). The virtio disk carries the device's exact
12-partition `blkdevparts=` A/B layout and the `/dev/block/by-name/` contract.

Where the seams meet: the guest runs the *real* binaries (static musl flared,
the LVGL fbdev UI); the RS485 bridge (`qemu/rs485-bridge/`) connects a QEMU
serial chardev to `sim/`'s `ModbusSlave`, so the register-level models serve as
the VM's field bus — behavior lives in one place, `sim/`, and the VM consumes
it. Scenario tests: `qemu/tests/boot-smoke.sh` (CI, in kernel-build),
`portal-scenario.sh` (check-in + OTA offer download against flare-edge's mock
portal), `ui-shot.sh` (QMP screendump + touch injection). Division of labour
with the other sims: NPU/RGA/HPMCU *behavior* stays `sim/`; UI *rendering
development* stays `lvglsim`; the VM is where processes, the kernel, and the
network meet. §5 still applies — no behavioural sim, this one included, catches
memory-map faults; and "boots under emulation" is never on-silicon evidence.

## 8. Order of Work

1. **Simulator core** — `membus`, `hpmcu`, the `cru` reset ladder, `modbus`, plus the
   `rga`/`npu` models. **Done.**
2. **C-driver MC/DC harnesses** — `relays.c` and `freshness.c` at 100% MC/DC, CI-gated
   via the shared `drivers/enforce-mcdc.sh`. **Done** (the first C coverage gate).
3. **flared devmem/hpmcu seam + tests** — firmware-side trait, unified with `sim/`
   once flare-edge consumes warden-sdk (a separate, maintainer-gated step). **Pending.**
4. **Config-lint CI gates** (§5) — the brick-class of bug. **Done.**
5. **Hermetic kernel build** (`build/build-kernel.sh` + the `patches-apply` gate). **Done.**
6. **Kernel 5.10→6.18.46 forward-port** (§6, ADR-0001). **Done** (hardware-verified).
7. **QEMU device sim** (§7, ADR-0006) — boot smoke, A/B disk harness, RS485 bridge,
   portal/watchdog/clock scenarios, display+touch. **Done** (emulation-verified;
   booting the real flare-edge rootfs+oem image pair is a documented later milestone).
