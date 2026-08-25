# RV1106 → 6.18 forward-port — live status

Target: **Linux 6.18.46** (vanilla, `flare-edge/research/linux-6.18.46/`), forward-ported
from the **vendor 5.10.160** tree, no plan44 code, built with our
`arm-rockchip830-...-gcc 8.3` (confirmed: **gcc 8.3 builds 6.18 fine**).

## Confirmed feasibility facts
- gcc 8.3 (our SDK toolchain) compiles the 6.18 kernel — no toolchain bump needed to start.
- `multi_v7_defconfig` (ARM + rockchip) configures; `CLK_RV1106=y` wires in cleanly.
- rv1106's siblings **rv1126/rv1108 exist in both trees**, so their 5.10→6.18 delta is a
  working template for the framework API changes.

## M1 — clock driver (`clk-rv1106.c`, 1294 lines): ✅ COMPILES CLEAN on 6.18
Build-fix loop against 6.18 — `clk-rv1106.o` (85732 bytes) builds with no errors.
Fixed (captured in `clk/`):
1. **Kconfig + Makefile hooks** — added `CONFIG_CLK_RV1106` (mirrors CLK_RV1126).
2. **CRU register macros** — ported all 55 `RV1106_*` register-accessor `#define`s from the
   vendor `clk.h` into 6.18's `clk.h` (mainline has none).
3. **Header split** — `panic_notifier_list` moved to `<linux/panic_notifier.h>` (kernel
   5.18); added the include.
4. **`rockchip_clk_register_armclk` signature change** — 5.10 took
   `(num_parents, parent_clk, alt_parent_clk)`; 6.18 takes `(parent_names[], num_parents)`
   and drives the mux from `reg_data.mux_core_main/alt`. Adapted the call to the 6.18 form
   using a parent-names array (sibling-delta from rv1126). **⚠ PORT-VERIFY**: the mux input
   list `{ "gpll","cpll","apll" }` and the `mux_core_main/alt=2` mapping are a best-effort
   from the 5.10 intent (main=apll) — they set the **CPU clock source**, so they must be
   checked against the RV1106 `CORECLKSEL_CON` register map (TRM) and validated on hardware
   before trusting a boot. A wrong mux silently breaks boot.

5. **`CLK_FRAC_DIVIDER_NO_LIMIT`** — Rockchip downstream-only frac-divider flag (6 uses on
   the UART frac clocks); mainline has no min/max opt-out, mapped to 0 (default limit).
   **⚠ PORT-VERIFY**: UART fractional baud accuracy.

## M1 — pinctrl (`pinctrl-rockchip.c/.h`): ✅ COMPILES CLEAN on 6.18
`pinctrl-rockchip.o` (173688 bytes) builds no-errors. Transplanted from vendor 5.10 (effort S,
zero API drift — the survey's assessment held): added `RV1106` to the type enum; a 159-line
block of `RV1106_DRV/PULL/SMT_*` macros + 3 `rv1106_calc_*_reg_and_bit()` functions; `case
RV1106:` in the 3 pull functions + the RK3568 drive-strength group; `rv1106_pin_banks[]` +
`rv1106_pin_ctrl`; and the `rockchip,rv1106-pinctrl` of_device_id (dropping the vendor's
`#ifdef CONFIG_CPU_RV1106` guard — mainline compiles all SoCs unconditionally). Captured as
`pinctrl/0001-rv1106-pinctrl.patch`.
- **PORT-VERIFY RESOLVED (iomux offsets):** the DRV/PULL/SMT per-bank offsets are confirmed by
  a THIRD independent source — the upstream Simon Glass v3 pinctrl patch. Its
  `rv1106_{drv,pull,smt}_offsets[]` match ours exactly once the 0x10000-strided per-bank IOC
  base is applied (e.g. vendor GPIO2 DRV `0x100C0` = `0x10000 + 0xc0`). Retires the "do not
  guess" iomux risk without hardware. (Note upstream uses per-bank IOC regmaps + low offsets;
  we use the vendor's full-offset form — same absolute register, both compile.)
- **Still PORT-VERIFY:** GPIO4 bank pin-count (`pin_banks` says 24, DT `gpio-ranges` says 32) —
  carried from vendor unchanged; needs TRM/hardware.

## M1 — mach: ✅ DONE
`mach-rockchip` RV1106/RV1103 SoC recognition added as a DT-compat entry (no
`CPU_RV1106` symbol recreated). Captured as `mach/0001-rv1106-soc-recognition.patch`.
**M1 is complete: clk + pinctrl + mach all compile clean on 6.18.**

## M2 — earlycon build: ✅ DONE (boot pending hardware)
The first full kernel build with our SoC drivers, 2026-08-24:
- **`multi_v7_defconfig` + `configs/m2-earlycon.fragment` builds an 11.8 MB zImage**
  with `clk-rv1106.o` (85732 B) and `pinctrl-rockchip.o` (173688 B, our rv1106 data)
  compiled *into the full tree* — no rv1106 warnings/errors. Reproducible via
  `build-m2.sh`.
- **`dts/rv1106-warden-m2.dts` → `rv1106-warden-m2.dtb` compiles clean** (W=1, no dtc
  warnings). A deliberately minimal DT — CPU (cortex-a7), GIC-400, arch timer, 256 MiB
  RAM, GRF, the CRU (our clk-rv1106), and uart2 (`snps,dw-apb-uart`, the console) —
  with `earlycon=uart8250,mmio32,0xff4c0000` so the first print reuses the loader's
  divisor before any clock/pinctrl probe.

**Still PORT-VERIFY before a boot is trusted:** memory size/base (256 MiB @ 0x0 assumed
— the loader usually patches this); the CPU clock mux in clk-rv1106 (§M1 item 4); the
console baud (1.5M assumed). A wrong DDR/clock value silently hangs before or just after
earlycon.

## M2 — boot: NEXT (on hardware)
Package `zImage` + `rv1106-warden-m2.dtb` into a boot image and load on a bench unit
(c8a3 / w7159, both with a proven recovery net), watching the debug UART — or the
coprocessor USB console (R5) — for the earlycon "it's alive". A hang is expected to be
one of the PORT-VERIFY values above; iterate with recovery ready. This is an attended
step (brick-adjacent), not an unattended one.

Then M3: `dw_mmc` DT + rootfs on our Buildroot userspace.

## Upstream tracking (decision 2026-08-23)
Base stays the vendor forward-port (applies to 6.18; we control it). The unmerged upstream
RV1106 series (~35 patches, "New" state, does NOT apply cleanly to 6.18 — pinctrl v3 failed at
`pinctrl-rockchip.c:3390`) is used as a **correctness oracle** for boot-critical PORT-VERIFY
values (already retired the iomux offsets) and tracked for eventual convergence when it merges.

## Honest scope
This is the first driver of ~120 in the RV1106 BSP. The clk port alone is a multi-cycle
effort of the four delta classes above; the whole bring-up (M1–M6 in `../docs/bringup.md`)
is a multi-month kernel effort. Progress is real and the method is proven; correctness of
boot-critical pieces (clock mux, PLL rates, DDR, pinctrl iomux) requires the TRM and
on-hardware validation — which waits on c8a3's recovery and, ultimately, careful bring-up.

## Layout
```
clk/        clk-rv1106.c + rv1106-cru.h + framework-hooks patch (M1)
pinctrl/    0001-rv1106-pinctrl.patch                              (M1)
mach/       0001-rv1106-soc-recognition.patch                     (M1)
dts/        rv1106-warden-m2.dts   minimal earlycon board DT       (M2)
configs/    m2-earlycon.fragment   defconfig delta over multi_v7   (M2)
build-m2.sh reproducible M2 build (zImage + dtb)
```
The full ported tree lives in `flare-edge/research/linux-6.18.46/` (scratch); this dir is
the durable, reviewable capture, to become a proper patch series as milestones land.
