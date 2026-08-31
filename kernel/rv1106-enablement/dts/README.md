# RV1106 6.18 devicetree

- **`rv1106-warden.dts`**: our board DT. `#include`s the vendor `rv1106.dtsi`
  (the full SoC: pinctrl, gpiox5, dmac, cru+grf_cru, uart, emmc, i2c, saradc,
  watchdog, usb, vop, npu, rga, ...), then enables only what the wall-HMI needs.
  No camera/ISP/CSI. Grows one driver batch at a time (see `../DRIVER-PARITY.md`).
- **`rv1106-warden-m2.dts`**: the earlier standalone minimal DT used to bring up
  M2/M3 before the full-SoC transplant; kept for reference.

## Transplanting the vendor SoC DT onto 6.18 (recipe)

Copy into `arch/arm/boot/dts/rockchip/` of the 6.18 tree:
`rv1106.dtsi`, `rv1106-pinctrl.dtsi`, `rockchip-pinconf.dtsi`. Copy the
vendor-only dt-bindings headers into `include/dt-bindings/`:
`soc/rockchip-system-status.h`, `suspend/rockchip-rv1106.h`,
`display/media-bus-format.h`, and **overwrite** `soc/rockchip,boot-mode.h`
with the vendor's (it has extra BOOT_CHARGING/UMS/PANIC/WATCHDOG constants the
DT uses). Then the two hardware deltas:

1. **Delete the `psci` node** from `rv1106.dtsi`. RV1106 has no secure monitor in
   our boot chain, so `arm,psci-1.0` + `method="smc"` makes the SMC call fault ->
   `Oops - bad mode` -> `Attempted to kill the idle task`. (Confirmed on c8a3.)
2. Enable the peripherals in the board DTS (`&uart2`, `&emmc`, ...).

Verified on warden-c8a3: pinctrl, gpio0-4, pl330 DMA, uart1/2/4, i2c3, dw-wdt all
probe; full WardenOS userspace boots.
