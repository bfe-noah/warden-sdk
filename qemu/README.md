# The Device Simulator

A QEMU virtual machine that boots the real forward-ported kernel and real
userspace: the 86 Panel — init, daemons, networking, OTA, watchdog, display —
developed and tested with no board attached. The third simulator in the stack
(three-way split: root README), deliberately not named "sim": it runs the
whole machine above the kernel entry point on real binaries — bring your own,
or drop prebuilt payloads in `payload/`. Decision record: ADR-0006.

## The Boundary

There is no RV1106 machine model in QEMU and everything below the kernel is
closed rkbin blobs plus mask ROM, so the VM **enters at `-kernel zImage`** on
`-M virt,highmem=off` (single Cortex-A7, 256M — the RV1106G3's shape).

| Emulated / substituted | Not emulated (stays bench / `sim/` territory) |
|---|---|
| Kernel boot, init ordering, switch_root | BootROM, idblock/DDR-init, SPL, U-Boot |
| A/B *outcome* (`warden.slot=` cmdline) | Real BCB A/B selection, bootcount auto-revert |
| Storage: virtio-blk with the device's exact `blkdevparts=` layout + `/dev/block/by-name/` contract | eMMC controller itself |
| Network: virtio-net (slirp, hostfwd 22/80/28443) | GMAC, AIC8800 wifi, usb0 gadget |
| Display: virtio-gpu 720x720 via fbdev emulation | VOP/RGB666 pipeline, CH32V003 panel init, RGA blits |
| Touch: virtio-tablet (QMP `input-send-event`) | GT911 on I2C3 |
| Watchdog: i6300esb (PCI), `-action watchdog=reset` | DW watchdog @0xff5a0000, HPMCU supervisor |
| RS485: pci-serial chardev bridged to `sim/`'s `ModbusSlave` | Real UART4 timing/electrical behavior |
| RTC: PL031 (`--rtc` reproduces the no-RTC 2021-clock incident class) | The unpopulated backup-cell reality |

**"Boots/works under emulation" is never evidence of "works on silicon."**
The VM narrows which claims need a panel; on-device claims still need
on-device evidence. Conversely, the VM is the first environment that runs
production binaries on a non-RV1106 memory map — it found flare-edge #106
(fatal SIGBUS in flared's HPMCU probe) and #107 (Y2038 time_t truncation)
on its first two boots of real userspace.

Documented guest deviations from production, set by stage-2 init:
`WARDEN_FLARE_INSECURE=1` (the desk mock portal is plain HTTP) and
`WARDEN_HPMCU=0` (no mailbox SRAM on virt; flared >= flare-edge#106 fix
required, or the daemon dies of SIGBUS).

## Quick Start

```sh
# 1. kernel: canonical build boots the VM as-is; the fragment variant adds
#    the scenario devices (PCI serial, watchdog, WireGuard, virtio-gpu/input)
WORK=$HOME/kbuild-out CROSS_COMPILE=arm-linux-gnueabihf- \
  WARDEN_KCONFIG_FRAGMENT=qemu/configs/virt.fragment bash build/build-kernel.sh

# 2. initramfs (sha256-pinned static busybox + qemu/rootfs/) and A/B disk
bash qemu/mkinitramfs.sh
bash qemu/mkimage.sh                  # options: --portal-url --state K=V --fw-version

# 3. run (see run.sh header for all flags)
bash qemu/run.sh --kernel $HOME/kbuild-out/linux-6.18.46/arch/arm/boot/zImage --shell
```

Payload: drop static musl armv7 binaries into `qemu/payload/` (see its
README) — `warden-flared`, `warden-modbus`, and `warden-ui` (the LVGL
fbdev+evdev build from flare-edge `tools/build-ui-vm.sh`) are started by
stage-2 init when present.

## Scenarios

All take the virt-fragment `<zImage>`; `FLARE_EDGE=<checkout>` where noted.

| Scenario | Needs | Proves |
|---|---|---|
| `boot-smoke.sh` | — | sentinel-asserting boot; runs in CI inside kernel-build |
| `portal-scenario.sh` | `FLARE_EDGE` | real flared against the desk mock portal: authenticated check-in, desired-state pull, signed tier-1 `.wfw` download; verify/stage/APPLYING as a dry run (no `WARDEN_FW_ALLOW_APPLY`) |
| `ota-apply.sh` | `FLARE_EDGE` | the FULL apply: the `.wfw`'s bootable rootfs payload is written to rootfs_b (`run.sh --allow-apply` gates it per boot), the AvbABData in `misc` flips, and slot `_b` boots the applied version |
| `ui-shot.sh` | — | display+touch, headless: QMP-screendumps the 720x720 UI, taps the Metrics tab via `input-send-event`, asserts the frame changed (`qmp.py` is the QMP client) |
| `real-image-boot.sh` | matched `rootfs.img` + `oem.img` | an ACTUAL flare-edge build (placed by `mkimage.sh --rootfs-image/--oem-image`) boots its own init chain to getty; binaries predating known fixes reproduce their bugs faithfully — a time machine for field issues |
| watchdog (`run.sh --watchdog`) | — | arm `/dev/watchdog`, don't pet: the VM resets ~30 s later (verified) |

Scenario fine print:

- OTA: the BCB slot CHOICE and the physical reset stay emulated by the
  harness (ADR-0006 boundary); the VM exports `WARDEN_HARD_RESET=0` so
  flared's post-apply reset surfaces as a reported error, not a /dev/mem
  fault.
- Touch injection holds 200 ms — an instantaneous press+release lands inside
  one LVGL poll and never clicks.
- Watchdog + a flared payload don't mix: flared pets only while the UI
  heartbeat is fresh.

## Gotchas

- AF_UNIX socket paths cap at ~108 chars — keep `--rs485`/`--qmp` paths short.
- A serial port that is closed discards incoming bytes: hold ONE fd open
  across write and read when scripting the guest side of the RS485 bridge.
- `highmem=off` and `-global virtio-mmio.force-legacy=false` are load-bearing
  (32-bit ECAM reach; virtio-1-only gpu/input) — both live ONLY in run.sh,
  which every script (boot smoke included) delegates to.
- Never pass `earlyprintk`: DEBUG_UART_PHYS is the RV1106's 0xff4c0000.

## Requirements

`qemu-system-arm` (Debian 13 ships QEMU 10), `curl`, `cpio`, `mkfs.ext4`,
`gcc-arm-linux-gnueabihf` (kernel build), `python3` (+`cryptography` for the
portal scenario's `.wfw` signing). CI: the hosted `qemu-tools` job builds the
tooling; the boot smoke runs inside the (also hosted, dispatch-only)
`kernel-build` job, which apt-installs its own toolchain and qemu (ADR-0007).
