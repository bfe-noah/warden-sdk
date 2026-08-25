# warden-sdk

[![ci](https://github.com/bfe-noah/warden-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/bfe-noah/warden-sdk/actions/workflows/ci.yml)
![Lines of code](.github/badges/loc.svg)
![Tests](.github/badges/tests.svg)
![Coverage](.github/badges/coverage.svg)

The build, driver, and simulation SDK for WardenOS (the Luckfox Pico 86-Panel /
RV1106 HMI). A from-scratch replacement for the twice-ported vendor stack
(Rockchip SDK → Luckfox SDK → our patched fork), built to the same standard as
the rest of the firmware: tested, benchmarked, reproducible, and honest about
what runs on real silicon versus what we simulate.

> Status: **bootstrapping.** This repo is being stood up incrementally; today it
> hosts the hardware **simulator** and its tests. The kernel forward-port and the
> hermetic image build move in as each is proven. Until then, flare-edge still
> builds firmware from the vendored SDK + `sdk-patches/`; nothing here is on the
> production build path yet.

## Why a new SDK

The vendored SDK is a ~2 GB opaque fork of a fork. Our real changes to it lived,
until recently, as uncommitted edits in one working copy (`flare-edge/sdk-patches/`
is the tracked form). It bakes absolute paths, needs `python` (not python3),
silently drops Kconfig options, and — the failure that motivated this repo — gives
us **no way to test hardware-dependent code off the device.** Every driver change
had to be validated by flashing a panel. That is slow, and it is dangerous: it is
how a boot-loaded-watchdog change bricked a bench unit (the load address collided
with unreserved kernel RAM — a mistake a target-config check or a memory-map model
would have caught before any flash).

The SDK's job is to make the firmware **buildable, testable, and hardenable
without a panel in the loop**, and to move us onto a modern, maintained kernel.

## Goals (from future-features)

1. **Modern kernel.** A self-built **Linux 6.18.46**, forward-ported directly from
   the vendor 5.10.160 tree (no plan44/OpenWrt code) on our current Buildroot LTS
   (2025.02.x). This is **done and hardware-verified on `warden-c8a3`**: essentially
   every RV1106 block the 86-Panel uses boots and works — clk, pinctrl, eMMC, GMAC,
   TRNG, OTP, SARADC/TSADC, RTC, USB host, PWM/backlight, **VOP display**, **GT911
   touch**, **AIC8800 wifi**, **RGA**, **I2S audio**, **HPMCU mailbox**, the **open
   NPU driver**, and **PVTM**. Mainline was not viable (no DT/clk/display/RGA/NPU/
   flash-boot upstream for RV1106); the direct 5.10→6.18 forward-port reuses the
   already-in-mainline rv1126 register data where it matches and carries our deltas
   as a reviewable patch series (`patches/`).
2. **Ported, hardened drivers → 100% MC/DC on the code we own.** "100% MC/DC on
   100% of drivers" is infeasible as literally stated: ~97% of driver LOC is
   vendor blobs (the AIC8800 wifi driver alone is 88.5K lines). So the target is
   **tiered**: real MC/DC on *our* hardware code (modbus master, relays, RGA
   wrapper, HPMCU supervisor, devmem/reset ladder); fault-injection + branch
   hardening for the vendor blobs behind a stable seam.
3. **A proper simulator.** Simulate the hardware the vendor SDK cannot: **RGA**
   (2D blitter), the **RISC-V HPMCU** coprocessor, and the **NPU** — plus the
   register/SRAM (`/dev/mem`) and sysfs surfaces the drivers touch — so driver and
   supervisor logic runs and is tested on the host, in CI, with no panel.
4. **Its own repo, held to firmware standards.** Tests, benchmarks, reproducible
   builds, CI. This repo.

## Architecture — one seam, two backends

The organizing idea is a thin **Hardware Abstraction Seam** per hardware block.
Firmware code talks to the seam (a trait in Rust, a function table in C); the seam
has two backends:

```
        firmware / driver logic
                  │
          Hardware Abstraction Seam        (devmem, hpmcu, rga, npu, modbus, gpio)
             ┌────┴────┐
        real backend   sim backend
      (/dev/mem, ioctl, (software model,
       /proc, serial)    host-testable)
```

- **On-device**, the seam binds the real backend (mmap `/dev/mem`, `librga`
  ioctls, the serial port, `/proc/rknpu`).
- **On the host**, it binds the **sim backend** — a faithful software model of the
  block. The HPMCU sim, for example, runs the SCR1 watchdog firmware's exact state
  machine (boot-grace, heartbeat-timeout, fire) against an in-memory mailbox, so
  the flared supervisor's arm/beat protocol is exercised end-to-end in a unit test.

The seam is the same object the driver-hardening effort measures MC/DC against,
and the same object the simulator implements — so the two goals reinforce rather
than duplicate each other.

## Layout

```
sim/        the hardware simulator (Rust): mailbox/devmem model, HPMCU, RGA, NPU.
drivers/    our own hardened drivers + their seams (as they migrate in).
patches/    the vendor-SDK delta (mirrors flare-edge/sdk-patches until it moves here).
build/      the hermetic image-build wrapper (kernel → rootfs → image), incremental.
ci/         CI: patches-still-apply, host tests, coverage, benchmarks.
docs/       architecture + ADRs (decisions/).
tools/      dev tooling. config-lint: static target-config gates (MCU-load-vs-reserved-memory — the 0x40000 brick class).
```

## Principles

Evaluated against the stack philosophy — **openness, hardness, modernness**:

- **Open** over closed where we can: `rkdeveloptool` over the closed `upgrade_tool`;
  source-buildable `librga` over blobs where a source path exists; the simulator is
  fully open and ours.
- **Hard**: every seam has a fault-injection path (a wedged SDIO link, a stalled
  MCU, an RGA timeout) so recovery code is tested against failure, not just success.
  On-device claims still need on-device evidence; the sim narrows *which* claims
  need a panel, it does not replace that rule.
- **Modern**: newest kernel we can actually run; current Buildroot LTS; Rust for new
  host-testable code; reproducible builds.

## Relationship to flare-edge

flare-edge (WardenOS: the LVGL UI + the `flared` daemon) is the product; warden-sdk
is what builds and tests it. During bootstrap, flare-edge consumes warden-sdk piece
by piece: first the simulator (as a dev/test dependency), later the image build.
No flare-edge code moves here — only the SDK/build/sim/driver-seam layer.
