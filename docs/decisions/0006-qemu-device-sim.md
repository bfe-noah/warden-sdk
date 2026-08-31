# ADR 0006 — QEMU Device Simulator

**Status:** Accepted (2026-08-29).

## Context
The two existing simulators cannot test the *device*: `lvglsim` (flare-edge)
is an SDL rendering harness, and `sim/` models registers behind driver seams.
Init ordering, the daemons as real processes, networking/enrollment against
FLARE, OTA, and the watchdog were testable only on a bench panel — flare-edge's
fault suite marks five scenarios "HIL, human prompts", its Playwright e2e needs
a live panel on the LAN, and its OTA desk test stops at "reached APPLYING".

Two ways to emulate the panel were considered:

1. **A custom RV1106 QEMU board model.** Nothing exists upstream or in the
   community, so this means writing VOP/CRU/GRF/eMMC/HPMCU device models from
   scratch and maintaining them against QEMU — months of work that duplicates
   what `sim/` already models in Rust. It still could not run the boot chain:
   BootROM is mask ROM and the DDR-init/idblock stages are closed rkbin blobs.
2. **The generic `-M virt` machine, entering at `-kernel zImage`.** The
   forward-ported 6.18.46 config is multi_v7-derived and already carries
   `ARCH_VIRT` plus the full virtio set — the canonical RV1106 zImage boots
   virt **unmodified** (verified 2026-08-29). Peripherals become virtio
   substitutes; SoC-block behavior stays in `sim/`, bridged in (the RS485
   chardev bridge) rather than re-modeled.

## Decision
Option 2. `qemu/` holds the harness: `qemu-system-arm -M virt,highmem=off
-cpu cortex-a7 -smp 1 -m 256M` (the RV1106G3's shape), one canonical kernel
image plus an optional additive config fragment (`qemu/configs/virt.fragment`
via the `WARDEN_KCONFIG_FRAGMENT` hook — PCI/pci-serial/i6300esb/WireGuard/
virtio-gpu/virtio-input; the RV1106 build is byte-identical with the variable
unset). The VM carries the device's real 12-partition `blkdevparts=` A/B
layout on a virtio disk and populates the `/dev/block/by-name/` contract.
The name is `qemu/`, not any variant of "sim" — the wikis already warn that
"sim" is two different things.

Notable mechanics: `highmem=off` because the non-LPAE 32-bit kernel cannot
reach virt's default 40-bit PCIe ECAM; `-global virtio-mmio.force-legacy=false`
because virtio-gpu/input are VERSION_1-only devices.

## Consequences
- Everything below the kernel is **out of scope**: BootROM, idblock, U-Boot,
  the real BCB-driven A/B selection and bootcount auto-revert. The initramfs
  `warden.slot=` switch emulates U-Boot's *choice*, not the mechanism. The
  untested A/B rollback chain stays bench territory.
- Display, input, network, and storage are **substitutes** (virtio), not
  models. "Boots/works under emulation" is never evidence of "works on
  silicon"; the VM narrows which claims need a panel.
- The VM is the first environment that runs production userspace binaries on
  a non-RV1106 physical memory map, which makes it a canary for baked-in
  hardware assumptions (it immediately found flare-edge #106, a fatal SIGBUS
  in flared's HPMCU probe, and #107, a Y2038 time_t truncation in the UI).
- The guest deliberately deviates from production in documented ways
  (`WARDEN_FLARE_INSECURE=1` for the desk mock portal, `WARDEN_HPMCU=0`);
  qemu/README.md carries the emulated-vs-not table.
- The kernel-build CI job gains a fail-closed qemu boot smoke; hosted runners
  build (but cannot boot) the initramfs and disk image.
