# warden-sdk

[![ci](https://github.com/blueflare-energy/warden-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/blueflare-energy/warden-sdk/actions/workflows/ci.yml)
![Lines of code](.github/badges/loc.svg)
![Tests](.github/badges/tests.svg)
![Coverage](.github/badges/coverage.svg)

A modern, open development environment for the **Luckfox Pico 86 Panel**
(Rockchip RV1106): a current Linux kernel as a reviewable patch series, a
hermetic build, a QEMU device simulator, register-level hardware models, and
MC/DC-hardened drivers (Modified Condition/Decision Coverage — the
avionics-grade test bar). It replaces the vendor stack — a ~2 GB, twice-forked
SDK pinned to Linux 5.10 — with tooling that is tested, benchmarked,
reproducible, and honest about what runs on real silicon versus what is
simulated.

Originally built as the SDK for WardenOS (BlueFlare Energy's wall-panel
firmware), but nothing here requires it: if you have an 86 Panel, this repo
gives you a modern kernel and a way to develop for the board without flashing
it on every change.

## Why

The vendor SDK bakes in absolute paths, silently drops Kconfig options, and
offers **no way to test hardware-dependent
code off the device** — every change means flashing a panel. That is slow and
occasionally destructive: a coprocessor load address that collided with
unreserved kernel RAM bricked a bench unit, a mistake a static memory-map
check would have caught before any flash (`tools/config-lint` now is that
check). This SDK exists so the board is buildable, testable, and hardenable
**without a panel in the loop**, on a maintained kernel.

## What Works

A self-built **Linux 6.18.46**, forward-ported from the vendor 5.10.160 tree
as a subsystem-split patch series (`patches/`) and hardware-verified on a
bench panel: clk, pinctrl, eMMC, GMAC, TRNG, OTP, SARADC/TSADC, RTC, USB
host, PWM/backlight, VOP display, GT911 touch, AIC8800 wifi, RGA, I2S audio,
the HPMCU (RISC-V watchdog coprocessor) mailbox, the open NPU driver, and
PVTM. Mainline alone was not
viable (no RV1106 device tree, clock, display, RGA, NPU, or flash-boot support
upstream); see `docs/decisions/0001-kernel-base.md`.

## Quick Start

Requirements: `gcc-arm-linux-gnueabihf`, `qemu-system-arm`, `curl`, `cpio`,
`mkfs.ext4`, a bare `python` on PATH (Debian/Ubuntu: `python-is-python3`),
gcc >= 14 (for the MC/DC gate), and Rust (for the simulators' tests).

```sh
# 1. Build the kernel: fetch pinned pristine 6.18.46, apply patches/, emit
#    zImage + rv1106-warden.dtb. WORK must sit outside any git checkout.
WORK=$HOME/kbuild-out CROSS_COMPILE=arm-linux-gnueabihf- bash build/build-kernel.sh

# 2. Boot it in the QEMU device simulator (no hardware needed):
bash qemu/mkinitramfs.sh
bash qemu/mkimage.sh
bash qemu/run.sh --kernel $HOME/kbuild-out/linux-6.18.46/arch/arm/boot/zImage --shell

# 3. Run the test suites:
for d in sim tools/config-lint qemu/rs485-bridge; do
  (cd "$d" && cargo test)
done
for d in drivers/*/test; do make -C "$d" check; done  # 100% MC/DC gate (gcc >= 14)
```

The kernel variant with the simulator's extra devices (PCI serial, watchdog,
WireGuard, display) adds one env var to step 1:
`WARDEN_KCONFIG_FRAGMENT=qemu/configs/virt.fragment`. See `qemu/README.md`
for the scenario tests (portal, OTA apply, display + touch, watchdog).

## Layout

```
patches/    the RV1106 forward-port onto pristine linux-6.18.46 (subsystem-split)
build/      the hermetic kernel build (fetch pinned source -> apply patches -> zImage + dtb)
qemu/       the device simulator: QEMU -M virt boots the real kernel and real userspace;
            A/B disk layout, RS485 bridge into sim/, scenario tests
sim/        register-level hardware models (Rust): membus, HPMCU, CRU, Modbus, RGA, NPU
drivers/    hardened hardware-facing drivers with HAL seams and 100% MC/DC harnesses
kernel/     forward-port provenance and bring-up records (point-in-time; patches/ is canonical)
tools/      config-lint (static memory-map gates) and dev tooling
docs/       architecture, ADRs (decisions/), CI/CD
```

## Architecture

One thin **hardware abstraction seam** per block (a trait in Rust, a function
table in C). Firmware logic talks to the seam; the seam binds a real backend
on the device (`/dev/mem`, ioctls, serial, `/proc`) or a simulated backend on
the host. The same seam is what the driver-hardening effort measures MC/DC
against and what the simulator implements, so the two reinforce rather than
duplicate each other. Full detail: `docs/architecture.md`.

Three simulators, by design not one:

| Simulator | What it runs | What it proves |
|---|---|---|
| `sim/` | register-level Rust models | driver and supervisor logic, with fault injection |
| `qemu/` | the real kernel + real userspace on `-M virt` | boot, init, daemons, networking, OTA, watchdog, display + touch |
| `lvglsim` (downstream) | the LVGL UI on SDL | rendering and UI flows |

Boots and passes under emulation are never treated as on-silicon evidence;
the simulators narrow which claims need a panel, they do not replace it.

## Principles

- **Open**: open tools over closed ones (`rkdeveloptool`, source-built
  components, an open simulator); GPL-2.0-only.
- **Hard**: every seam has a fault-injection path — recovery code is tested
  against failure, not just success.
- **Modern**: the newest kernel the hardware can run, current toolchains,
  Rust for new host-testable code, reproducible builds.

## Downstream

WardenOS (the 86 Panel firmware this SDK was born for) consumes warden-sdk
from its own private repo, flare-edge; issue references and checkout paths
pointing there are context, not reachable links. The QEMU simulator runs its
production binaries unmodified — including real over-the-air updates against
a mock portal.

## License

**GPL-2.0-only**, repo-wide (see `LICENSE`; a per-file SPDX identifier
governs where present). The kernel material in `patches/` and `kernel/` is
derivative of the Linux kernel and GPL-2.0 vendor code; per-driver origin is
tracked in `kernel/rv1106-enablement/PROVENANCE.md`. Contributions are
accepted under the same license (inbound = outbound).
