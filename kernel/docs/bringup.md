# RV1106 → Linux 6.18 forward-port (self-built, from vendor 5.10)

**Decision (2026-08-23):** forward-port the RV1106 SoC enablement from the
Rockchip **vendor 5.10** tree straight to **Linux 6.18 LTS**, ourselves, using **no
plan44 code**, built on **our Buildroot 2025.02 LTS + uClibc**. We own the tree.

## The reality this rests on (verified, not assumed)

Vanilla mainline (6.6.93 and 6.18) has **zero** RV1106 support — no devicetree, no
clock driver, no pinctrl data, not one `rv1106` reference in `arch/arm` or
`drivers/`. So "the newest kernel" is a generic ARM kernel that cannot boot this
SoC. What we forward-port is the **entire RV1106 BSP**: ~120 files in the vendor
5.10 tree touch `rv1106` (DT, clk, pinctrl, mach/PM, VOP display, mmc, and the
out-of-tree AIC8800/RGA/MPP). This is a large, multi-milestone kernel effort;
it is delivered incrementally, console-first.

## Method (how each piece is ported)

- **Framework drivers → sibling-delta.** clk and pinctrl are driven by the shared
  rockchip framework (`drivers/clk/rockchip/clk.c`, `pinctrl-rockchip.c`). RV1106's
  siblings **rv1126 and rv1108 exist in BOTH vendor-5.10 and mainline-6.18**, so
  their 5.10→6.18 delta is the exact recipe for how the framework API changed —
  we apply that recipe to rv1106's data (`clk-rv1106.c` = 1284 lines;
  `rv1106-cru.h` clock IDs; the rv1106 pinctrl table). This turns "adapt to APIs
  we have to guess" into "copy a diff a sibling already proves."
- **SoC-unique code → forward-port + build-fix-build.** `mach-rockchip`
  (machine, `rockchip_hptimer`, `rv1106_pm`/`rv1106_sleep.S`), the CRU/GRF glue,
  and the board DT have no sibling; port directly and iterate on the 6.18 build.
- **Mainline-present drivers → just wire DT + clocks.** 8250-dw UART, dw_mmc
  (SDIO/eMMC), dw-apb i2c, rockchip gpio/pinctrl core, pwm, dw-wdt watchdog are all
  in mainline 6.18 — we do **not** port them, we supply the DT nodes + clock/reset
  phandles and let the mainline drivers bind.
- **Out-of-tree drivers → port the driver.** AIC8800 wifi (plan44 has none; ours),
  RGA/MPP kernel bits (assess vs need — the 86-Panel has no camera), carrying our
  Tier-1 SDIO-wakeup fix forward.
- **Our 4 kernel patches** (panel-mcu-reset/logo, watchdog-enable, usb-otg
  dual-role, recovery-splash) → re-apply against 6.18 (line offsets + some DT
  bindings changed).

## Bring-up milestones (console-first order)

- **M0 — buildable base.** 6.18.46 tree + our Buildroot/uClibc toolchain builds a
  generic ARM kernel; a `rv1106_defconfig` forward-ported from vendor 5.10.
- **M1 — SoC compiles.** `mach-rockchip` RV1106 select + `clk-rv1106` +
  rv1106 pinctrl data + `rv1106-cru.h` compile clean against 6.18 (no boot yet).
- **M2 — earlycon boot.** DT (`rv1106.dtsi` core) + clk + pinctrl + 8250-dw →
  kernel prints to the RV1106 UART. The first "it's alive."
- **M3 — rootfs boot.** dw_mmc + DT → eMMC/SD rootfs, userspace up on our Buildroot.
- **M4 — display.** VOP2 + the RGB666 720×720 panel + GT911 touch + PWM backlight →
  the LVGL UI renders.
- **M5 — wifi + our patches.** AIC8800 SDIO driver ported (+ Tier-1 fix); the 4
  custom kernel patches re-applied.
- **M6 — the rest.** RGA (UI accel), watchdog, HPMCU coprocessor, USB-OTG dual role.

## Regression (so a 6.18 port doesn't reintroduce the struct-ABI bug class)

The warden-sdk simulator + MC/DC harness are how we hold the line: `HpmcuSim` and
the register/`MemBus` seam test the supervisor/reset logic off the ported kernel;
the `relays-mcdc` pattern extends to each ported driver we own; and a
**config-lint CI gate** (the class that bricked c8a3 — a load address in unreserved
kernel RAM) checks the DT reservations against the drivers on every build.

## Layout (this repo)

```
kernel/
  rv1106-enablement/   the ported RV1106 BSP as it lands (patches/files on 6.18)
  docs/bringup.md      this plan
```
The 6.18.46 source + the vendor 5.10 tree stay in `flare-edge/research/` and
`flare-edge/.../sdk/` respectively (references, not committed here).
