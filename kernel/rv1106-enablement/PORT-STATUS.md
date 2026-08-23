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

## M1 — mach + defconfig: NEXT
Per the breadth survey's integration order: mach is a ~2-line DT-compat add to `rockchip.c`
(do NOT recreate the dropped `CPU_RV1106` symbol); then the minimal M2 defconfig; then the DT
(SoC nodes transplant + board-specific PORT-VERIFY: console UART, boot media, panel timings);
then the first full kernel build toward an earlycon boot.

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
clk/
  clk-rv1106.c                         the ported driver (WIP, compiles to :427)
  rv1106-cru.h                         the clock-id dt-bindings header (staged)
  0001-rv1106-clk-framework-hooks.wip.patch   Makefile/Kconfig/clk.h deltas vs vanilla 6.18
```
The full ported tree lives in `flare-edge/research/linux-6.18.46/` (scratch); this dir is
the durable, reviewable capture, to become a proper patch series as milestones land.
