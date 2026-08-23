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

**Next:** the other core bring-up components (pinctrl, mach/timer, DT, defconfig) — being
surveyed breadth-first in parallel to surface all their deltas before porting.

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
