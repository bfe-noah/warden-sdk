# bfe-core1106-sdk

[![ci](https://github.com/blueflare-energy/bfe-core1106-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/blueflare-energy/bfe-core1106-sdk/actions/workflows/ci.yml)
![Lines of code](.github/badges/loc.svg)
![Tests](.github/badges/tests.svg)
![Coverage](.github/badges/coverage.svg)

A modern, open development environment for the **Luckfox Pico 86 Panel**
(Rockchip RV1106): a current Linux kernel as a reviewable patch series, a
hermetic build, a QEMU device simulator, register-level hardware models, and
MC/DC-hardened drivers (Modified Condition/Decision Coverage — the
avionics-grade test bar). It replaces the vendor stack — a ~2 GB, twice-forked
SDK pinned to Linux 5.10 — and is honest about what runs on real silicon
versus what is simulated. Born as the SDK for WardenOS (BlueFlare Energy's
wall-panel firmware); nothing here requires it.

## Why

The vendor SDK:

- bakes in absolute paths and silently drops Kconfig options;
- offers no way to test hardware-dependent code off the device — every change
  means flashing a panel;
- let a memory-map mistake brick a bench unit (a coprocessor load address in
  unreserved kernel RAM) that a static check would have caught —
  `tools/config-lint` is now that check.

This SDK makes the board buildable, testable, and hardenable **without a
panel in the loop**, on a maintained kernel.

## What Works

A self-built **Linux 6.18.46**, forward-ported from vendor 5.10.160 as a
subsystem-split patch series (`patches/`) and hardware-verified on a bench
panel: clk, pinctrl, eMMC, GMAC, TRNG, OTP, SARADC/TSADC, RTC, USB host,
PWM/backlight, VOP display, GT911 touch, AIC8800 wifi, RGA, I2S audio, the
HPMCU (RISC-V watchdog coprocessor) mailbox, the open NPU driver, and PVTM.
Mainline alone was not viable for RV1106 (ADR-0001).

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

Add `WARDEN_KCONFIG_FRAGMENT=qemu/configs/virt.fragment` to step 1 for the
kernel variant with the simulator's extra devices; `qemu/README.md` has the
scenario tests (portal, OTA apply, display + touch, watchdog).

## Layout

| Directory | Contents |
|---|---|
| `patches/` | the RV1106 forward-port onto pristine linux-6.18.46, subsystem-split |
| `build/` | hermetic kernel build: pinned fetch → apply patches → `zImage` + dtb |
| `qemu/` | device simulator: QEMU `-M virt` boots the real kernel and real userspace |
| `sim/` | register-level hardware models (Rust): membus, HPMCU, CRU, Modbus, RGA, NPU |
| `drivers/` | hardened hardware-facing drivers: HAL seams, 100% MC/DC harnesses |
| `kernel/` | forward-port provenance and bring-up records (`patches/` is canonical) |
| `tools/` | `config-lint` (static memory-map gates) and dev tooling |
| `docs/` | architecture, ADRs (`decisions/`), CI/CD |

## Architecture

One thin **hardware abstraction seam** per block (a trait in Rust, a function
table in C): firmware logic talks to the seam; the seam binds a real backend
on the device or a simulated backend on the host. MC/DC is measured against
the same seam the simulator implements, so the two reinforce each other.
Full detail: `docs/architecture.md`.

| Simulator | Runs | Proves |
|---|---|---|
| `sim/` | register-level Rust models | driver and supervisor logic, with fault injection |
| `qemu/` | the real kernel + userspace on `-M virt` | boot, init, daemons, networking, OTA, watchdog, display + touch |
| `lvglsim` (downstream) | the LVGL UI on SDL | rendering and UI flows |

With the production UI binary in `qemu/payload/`, `run.sh --display on` opens
the panel's 720x720 screen in a window, mouse clicks landing as touch —
device and UI in one VM. Emulation results are never on-silicon evidence;
the simulators narrow which claims need a panel.

## Principles

- **Open** — open tools over closed ones; GPL-2.0-only.
- **Hard** — every seam has a fault-injection path; recovery code is tested
  against failure, not just success.
- **Modern** — the newest kernel the hardware can run, current toolchains,
  Rust for new host-testable code, reproducible builds.

## Downstream

WardenOS (the 86 Panel firmware this SDK was born for) consumes this repo
from its private repo, flare-edge; issue references and checkout paths
pointing there are context, not reachable links. The QEMU simulator runs its
production binaries unmodified — real over-the-air updates included.

## License

**GPL-2.0-only**, repo-wide (`LICENSE`; a per-file SPDX identifier governs
where present). `patches/` and `kernel/` are derivative of the Linux kernel
and GPL-2.0 vendor code; per-driver origin is tracked in
`kernel/rv1106-enablement/PROVENANCE.md`. Contributions are accepted under
the same license (inbound = outbound).
