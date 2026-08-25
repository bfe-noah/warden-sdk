# PVTM (Process-Voltage-Temperature Monitor) — ✅ VERIFIED on warden-c8a3 (2026-08-25)

Whole-driver port: mainline 6.18 has **no** rockchip pvtm driver; the vendor
`drivers/soc/rockchip/rockchip_pvtm.c` (GPL-2.0, 1046L) supports rv1106. Copied it in
+ `include/linux/soc/rockchip/pvtm.h`; Kconfig `ROCKCHIP_PVTM` + Makefile; `=y`.

## API-delta fixes (6.18)
- `struct thermal_zone_device` is now opaque → replaced `pvtm->tz->ops->get_temp(...)`
  with the public `thermal_zone_get_temp(pvtm->tz, &cur_temp)`.
- added `#include <linux/of_device.h>` for `of_match_device()`.
- copied the vendor-only header `linux/soc/rockchip/pvtm.h`.

## The real blocker (why it first probed silently)
The vendor of_match_table wraps the rv1106 entries in `#ifdef CONFIG_CPU_RV1106` — a
vendor per-SoC symbol that does **not exist** in mainline. Result: devices
(`ff240000.pvtm`, `ff390000.pvtm`) were created + the driver registered, but the
compatibles were compiled out of the match table, so nothing bound and probe never
ran (no dmesg at all). **Fix: drop the `#ifdef CONFIG_CPU_RV1106` guard** — our tree
only builds for rv1106, so the entries are unconditional. (Watch for this guard in any
other vendor driver ported by verbatim copy.)

## Evidence
`rockchip-pvtm ff240000.pvtm: pvtm@0 probed` + `ff390000.pvtm: pvtm@0 probed`;
`/sys/kernel/debug/pvtm/core/value = pvtm: 71682 90462`,
`/sys/kernel/debug/pvtm/pmu/value = pvtm: 35772` (ring-oscillator counts, the PVT
signal). Exports `rockchip_get_pvtm_value()` for future DVFS voltage margining.
