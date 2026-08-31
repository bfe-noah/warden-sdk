# Architecture

How the SDK makes the 86 Panel buildable, testable, and hardenable without a
board in the loop. The seam inventory below comes from a full survey of the
downstream WardenOS firmware (the SDK's first consumer), not aspiration; the
file paths in it point into that (private) codebase and serve as engineering
context.

## 1. The Problem

The firmware touches RV1106 hardware through a *grab-bag* of mechanisms, each
tested (or not) differently. Today:

| Block | Where | Access | Test seam today | Fails on host by |
|---|---|---|---|---|
| Registers / SRAM (CRU reset, HPMCU mailbox) | `flared/src/devmem.rs`, `hpmcu.rs` | `/dev/mem` mmap `peek/poke32` | **none**, zero tests | (would fault; not exercised) |
| HPMCU / RISC-V coproc | `flared/src/hpmcu.rs` | via devmem + firmware blob load | `WARDEN_HPMCU_FW` redirects the blob path only | env gate disables it |
| NPU load | `ui-src/.../sysmon.c` | read `/proc/rknpu/load` | **none**, literal path | file absent -> "NPU absent" |
| RGA (2D blit) | `ui-src/.../warden_rga.c` | `librga improcess` + dma-heap ioctl | compile-time `#if WARDEN_USE_RGA` | `#if` off -> LVGL software path |
| RS485 daemon | `warden-modbus/modbus_engine.c` | `open("/dev/ttyS4")` | recompile `-DRS485_PORT=<pty>` | (recompiled for a pty) |
| RS485 panel client | `ui-src/.../modbus.c` | `AF_UNIX` socket | `WARDEN_MODBUS_SOCK` env override | socket absent -> "unavailable" |
| Relays / GPIO | `ui-src/.../relays.c` | `/sys/class/gpio` sysfs | **none**, literal paths | path absent -> "unavailable" |
| Slot metadata | `flared/src/slotctl.rs` | `misc` partition + `/proc/cmdline` | `WARDEN_MISC_DEV`, `WARDEN_CMDLINE_FILE` env overrides | (redirected to scratch files) |

Three patterns coexist: **compile-time `#if`** (RGA), **env-override**
(modbus socket, misc dev, cmdline, hpmcu fw), and
**fails-soft-when-the-path-is-absent** (NPU, relays, devmem-would-fault). The
last is not a test seam: you can inject "absent", never "relay 1 is ON" or
"NPU at 80%". The SDK's job: **one deliberate seam per block**, each with a
real backend and a sim backend.

## 2. Seam Taxonomy

Two seam kinds cover everything above:

- **Register/SRAM seam -> a trait.** `MemBus` (`sim/src/membus.rs`):
  `peek32/poke32` at a physical address. Real backend = flared `devmem.rs`
  mmap; sim backend = `SimBus`. The HPMCU watchdog and the CRU reset ladder
  both ride this. **Built.**
- **Resource-path seam -> env-override + injection.** Every device/proc/sys
  path a driver opens resolves through one indirection
  (`warden_hw_path("npu.load")` in C, an env-overridable const in Rust),
  generalizing the existing `WARDEN_MISC_DEV`/`WARDEN_MODBUS_SOCK` pattern,
  so a test points it at a fake file/fifo the sim writes. No LD_PRELOAD, no
  fake mounts.

RGA stays compile-time: `#if WARDEN_USE_RGA` already isolates the
librga/dma-heap calls, and the sim backend (a recording `improcess` fake)
swaps in behind the same `#if`, so the *dispatch* logic is tested even though
the blit is modelled.

## 3. The Register Simulator

A host Rust library modelling the hardware the vendor SDK cannot, so driver
and supervisor logic runs in CI with no panel. All models are **done**:

| Model | What it is |
|---|---|
| `membus` | `MemBus` trait + `SimBus` in-memory bus (`Clone`, so two "cores" alias shared memory) |
| `hpmcu` | the watchdog coprocessor's state machine (boot-grace, heartbeat-timeout, disarm, fire) on a `SimBus` mailbox with a virtual clock; 7 tests incl. the arm-within-grace no-boot-loop safety property |
| `cru` | reset ladder against the known glb_srst_fst / DW-watchdog registers, plus boot-mode register semantics (survives warm reset, cleared by POR, the MaskRom recovery maneuver) |
| `modbus` | byte-in/byte-out RTU slave: CRC16 byte-identical to the master, FC 0x01-0x06/0x0F/0x10/0x11, exception replies, silent-drop and forced-NAK fault injection; doubles as the QEMU sim's field bus via `qemu/rs485-bridge/` (section 7) |
| `npu` | `/proc/rknpu/load` text model behind the path seam; the load-readout UI is host-testable; NPU *compute* is out of scope |
| `rga` | recording `improcess` fake with programmable `IM_STATUS`, exercising offload-dispatch and CPU-fallback; wired into the `rga_improcess` benchmark |

**Next:** MEI (0x2B/0x0E) Modbus identification; the Tier-2 driver *sources*
migrate in with the flare-edge unification (ADR-0005); their hardware ends
are already modelled here.

**Integration with flare-edge, landed 2026-08-31** (flare-edge PR #110):
`warden-sim` is a flared dev-dependency (through the vendored submodule), and
unification tests in flared's own suite pin its real arm/beat and
reset-ladder logic to `HpmcuSim`/`CruSim`.

## 4. Driver Hardening

"100% MC/DC on 100% of drivers" is infeasible literally (~97% of driver LOC
is vendor blobs; AIC8800 wifi alone is 88.5K lines), so the target is
tiered:

- **Tier 1, our own hardware code -> real 100% MC/DC.** Extract the unit
  behind a small injectable seam, mock its world, build
  `-fcondition-coverage`, enforce via the shared `drivers/enforce-mcdc.sh`
  (gcc-14 `gcov --conditions`) in the CI `mcdc` job (the proven flare-edge
  `tests/uboot-ab` method). **Done:** `relays.c` (40/40 conditions) and
  `freshness.c` (66/66). **Next in:** the modbus master and the RGA
  dispatch, with the ADR-0005 unification (`drivers/README.md`).
- **Tier 2, near-mainline small drivers -> branch coverage + fault
  injection.**
- **Tier 3, vendor blobs (AIC8800, MPP/ISP/RGA libs) -> fault-injection
  hardening behind the seam**, not MC/DC: e.g. the SDIO-wedge recovery is
  tested against an injected wedge; the blob itself is untestable.

Every seam gets a fault-injection mode (a wedged SDIO link, a stalled MCU
heartbeat, an RGA timeout, a GPIO write EIO) so recovery code is tested
against failure, not just the happy path.

## 5. Target-Config Checks

The brick was a *memory-map* fault: the boot-loaded MCU's load address
(`0x40000`) is a reserved carve-out on Thunder-Boot boards but plain kernel
RAM on the 86 Panel. No behavioural sim catches that; it takes a **static
check against the target devicetree**, owned here as CI gates so mistakes
are caught before a flash, not on the bench.

- **Built:** `tools/config-lint`: every `LOADERn=Hpmcu` `LOAD_ADDR` in the
  rkbin loader `.ini` must land inside a DT `reserved-memory` range. Its
  test suite encodes the c8a3 brick itself (see its README).
- **Next:** partition-table-vs-image-size; vermagic-vs-kernel.

## 6. Kernel Forward-Port

**Done and hardware-verified** on the bench panel; the full peripheral set
in the README's "What Works" boots.

- **Base:** a direct forward-port of the vendor 5.10.160 tree onto pristine
  6.18.46, keeping the Buildroot LTS/uClibc userspace.
- **Not plan44/OpenWrt 6.6** (ADR-0001): that fork swaps Buildroot for
  OpenWrt/musl and ships no AIC8800 kmod; a platform swap, not a port.
- **Not mainline alone:** no RV1106 DT/clk/display/RGA/NPU/flash-boot
  upstream; already-mainline rv1126 register data is reused where it
  matches.
- **Dominant risk** was the struct-ABI break (the VLAN saga), mitigated by
  shipping kernel moves as one matched boot+oem image, never a partial
  reflash.
- **Kept honest** by the hermetic `build/build-kernel.sh` and the
  `patches-apply` CI gate against pristine 6.18.46; provenance in
  `patches/README.md` and `kernel/rv1106-enablement/`.

## 7. Device Emulation

A QEMU VM (`-M virt,highmem=off`, one Cortex-A7, 256M: the RV1106G3's
shape) boots the real forward-ported kernel and real userspace, entering at
`-kernel zImage`: everything below is closed rkbin blobs plus mask ROM.
Details, scenarios, and the emulated-vs-not table: `qemu/README.md` and
ADR-0006.

- The canonical RV1106 zImage boots virt unmodified; an additive fragment
  (`qemu/configs/virt.fragment` via `WARDEN_KCONFIG_FRAGMENT`) adds the
  scenario devices (PCI serial, i6300esb watchdog, WireGuard,
  virtio-gpu/input for the 720x720 UI).
- The virtio disk carries the device's exact 12-partition `blkdevparts=`
  A/B layout and the `/dev/block/by-name/` contract.
- The guest runs the *real* binaries; `qemu/rs485-bridge/` connects a QEMU
  serial chardev to `sim/`'s `ModbusSlave`, so bus behavior lives once, in
  `sim/`, and the VM consumes it.
- Division of labour: NPU/RGA/HPMCU *behavior* stays `sim/`; UI *rendering
  development* stays `lvglsim`; the VM is where processes, the kernel, and
  the network meet. section 5 still applies (no behavioural sim catches memory-map
  faults), and "boots under emulation" is never on-silicon evidence.

## 8. Order of Work

1. **Simulator core**: `membus`, `hpmcu`, `cru`, `modbus`, `rga`, `npu`.
   **Done.**
2. **C-driver MC/DC harnesses**: `relays.c` + `freshness.c` at 100%,
   CI-gated. **Done.**
3. **flared devmem/hpmcu seam + tests**: unified with `sim/`. **Done**
   (2026-08-31, flare-edge PR #110).
4. **Config-lint CI gates** (section 5): the brick class of bug. **Done.**
5. **Hermetic kernel build** + the `patches-apply` gate. **Done.**
6. **Kernel 5.10 -> 6.18.46 forward-port** (section 6, ADR-0001). **Done**
   (hardware-verified).
7. **QEMU device sim** (section 7, ADR-0006): boot smoke, A/B disk harness, RS485
   bridge, portal/watchdog/clock/OTA scenarios, display+touch, real-image
   boot. **Done** (emulation-verified).
