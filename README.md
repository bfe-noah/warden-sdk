# bfe-core1106-sdk

[![ci](https://github.com/blueflare-energy/bfe-core1106-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/blueflare-energy/bfe-core1106-sdk/actions/workflows/ci.yml)
![Lines of code](.github/badges/loc.svg)
![Tests](.github/badges/tests.svg)
![Coverage](.github/badges/coverage.svg)

A modern, open development environment for the **Luckfox Pico 86 Panel**
(Rockchip RV1106), replacing the vendor SDK — and honest about what runs on
real silicon versus what is simulated.

| | Vendor SDK | This repo |
|---|---|---|
| **Kernel** | 5.10.160, twice-forked, frozen | **6.18.46** — a reviewable, subsystem-split patch series onto pristine upstream; full peripheral set (display, touch, wifi, audio, NPU, ...) hardware-verified on a bench panel |
| **Build** | ~2 GB tree, absolute paths baked in, Kconfig options silently dropped | one hermetic script: sha256-pinned source fetch, fail-closed patch apply and config fragments |
| **Off-device testing** | none — every change means flashing a panel | register-level hardware models (`sim/`) plus a QEMU device VM booting the real kernel, real daemons, and the real UI with display + touch |
| **Config safety** | memory-map mistakes reach hardware (one bricked a bench unit) | static gates (`tools/config-lint`) catch them before any flash |
| **CI** | none | hosted pipeline: tests, coverage, benchmarks, patch-apply gate, kernel build with an in-CI QEMU boot smoke |
| **Flashing tools** | closed (`upgrade_tool`) | open (`rkdeveloptool`) |
| **License** | mixed | **GPL-2.0-only**, with a per-driver provenance ledger |

## Quick Start

Requirements: `gcc-arm-linux-gnueabihf`, `qemu-system-arm`, `curl`, `cpio`,
`mkfs.ext4`, a bare `python` on PATH (Debian/Ubuntu: `python-is-python3`),
gcc >= 14 (driver harnesses), and Rust (for the simulators' tests).

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
for d in drivers/*/test; do make -C "$d" check; done  # driver harnesses (gcc >= 14)
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
| `drivers/` | hardened hardware-facing drivers: HAL seams, test harnesses |
| `kernel/` | forward-port provenance and bring-up records (`patches/` is canonical) |
| `tools/` | `config-lint` (static memory-map gates) and dev tooling |
| `docs/` | architecture, ADRs (`decisions/`), CI/CD |

## Architecture

One thin **hardware abstraction seam** per block (a trait in Rust, a function
table in C): firmware logic talks to the seam; the seam binds a real backend
on the device or a simulated backend on the host. The driver test harnesses
measure against the same seam the simulator implements, so the two reinforce
each other. Full detail: `docs/architecture.md`.

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

A private firmware repo (flare-edge) consumes this SDK; issue references and
checkout paths pointing there are engineering context, not reachable links.
The QEMU simulator runs its production binaries unmodified — real
over-the-air updates included.

## License

**GPL-2.0-only**, repo-wide (`LICENSE`; a per-file SPDX identifier governs
where present). `patches/` and `kernel/` are derivative of the Linux kernel
and GPL-2.0 vendor code; per-driver origin is tracked in
`kernel/rv1106-enablement/PROVENANCE.md`. Contributions are accepted under
the same license (inbound = outbound).
