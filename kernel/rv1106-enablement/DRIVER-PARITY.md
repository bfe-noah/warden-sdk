# RV1106 6.18 driver parity vs the 5.10 vendor kernel

Goal: every driver the panel's 5.10 kernel runs must work at ≥ parity on our
self-built 6.18. Source of truth = the running 5.10 system on warden-c8a3
(`lsmod` + `/proc/interrupts`, captured 2026-08-24). Verify each on hardware via
the A/B `_b`-slot loop (`../docs/m2-boot-on-c8a3.md`); ✅ means confirmed on
c8a3, not just compiled.

| Driver / node | 5.10 evidence | mainline? | 6.18 status |
|---|---|---|---|
| CRU clock (clk-rv1106) | — | ported | ✅ M2 |
| pinctrl-rockchip (rv1106) | — | ported | ✅ M2 |
| GIC-400 / arch_timer | arch_timer | mainline | ✅ M2 |
| dw_mmc (eMMC) | dw-mci | mainline | ✅ M3 |
| 8250 uart2 (console) | ttyS2 | mainline | ✅ M2 |
| GPIO (rockchip, ×5 banks) | gpio-rockchip | mainline | ✅ batch1 (chips 0–4) |
| DMA (pl330, ff420000) | ff420000.dma-controller | mainline | ✅ batch1 |
| uart1 / uart4 | ttyS1, ttyS4 | mainline | ✅ batch1 |
| I2C (dw-apb, ff460000=i2c3) | ff460000.i2c | mainline | ✅ batch1 (i2c-3) |
| watchdog (dw-wdt, ff5a0000) | ff5a0000.watchdog | mainline | ✅ batch1 (watchdog0) |
| thermal governor | rockchip_thermal | mainline | 🔨 gov up; tsadc node TODO |
| SARADC (ff3c0000) | ff3c0000.saradc | mainline (no rv1106 compat) | ⬜ needs compat fallback |
| GPIO_SYSFS (legacy /sys/class/gpio) | — | mainline (config) | ⬜ goodix script needs it |
| PWM (rockchip) | — | mainline (=m) | ⬜ batch2 =y (backlight) |
| RTC (rv1106-rtc) | — | **no mainline driver** | ⬜ port (low prio) |
| USB2 phy (inno, rv1106) | rockchip_usb2phy_* | mainline (no rv1106 data) | ⬜ port (data + tuning) |
| USB OTG gadget (DWC3, eth0) | eth0 | mainline dwc3 (needs phy) | ⬜ M6 (after phy) |
| USB host (xhci, usb1) | xhci-hcd:usb1 | mainline | ⬜ M6 |
| crypto (aes/ccm/ctr/arc4) | modules | mainline | ⬜ batch2 (config =y) |
| PSCI node (removed) | — | — | ✅ deleted (no secure monitor → SMC fault) |
| VOP display (ff990000) | ff990000.vop | ported (rv1126 sibling) | 🔨 binds+DRM+card0; connector WIP |
| PWM backlight (pwm1) | — | mainline (rk3328 fallback) | ✅ backlight up (brightness) |
| RGB666 720×720 panel | — | panel-dpi | 🔨 probes; bus_format + connector WIP |
| GT911 touch (goodix) | goodix, gt911 | mainline | ⬜ M4 (needs GPIO_SYSFS ✅ + node) |
| GPIO_SYSFS / crypto / CFG80211 | — | mainline (config) | ✅ =y (batch2) |
| AIC8800 wifi (bsp/fdrv) | aic8800_* | **out-of-tree** | ⬜ M5 |
| AIC8800 BT (btlpm) | aic8800_btlpm | **out-of-tree** | ⬜ M5 |
| NPU (rknpu, ff660000) | rknpu, ff660000.npu | **out-of-tree** | ⬜ M6 |
| RGA 2D (rga2) | rga2 | rockchip | ⬜ M6 |
| I2S audio | i2s | rockchip | ⬜ M6 |
| FIQ debugger (ttyFIQ0) | fiq_glue | rockchip | ⬜ optional (we use ttyS2) |

Legend: ✅ verified on hardware · 🔨 built, not yet verified · ⬜ not started.
