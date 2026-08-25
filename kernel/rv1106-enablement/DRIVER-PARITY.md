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
| tsadc thermal (ff3c8000) | rockchip_thermal | ported (data+init+macros) | ✅ soc-thermal reads 39.8°C |
| SARADC (ff3c0000) | ff3c0000.saradc | ported (2-ch v2 data) | ✅ iio:device0 reads 2ch (adc-keys); fixed -22 via vref-supply |
| TRNG (rng@ff448000) | rockchip,trngv1 | mainline (rk3588 IP) | ✅ /dev/hwrng, real entropy (`rng-otp/`) |
| OTP/nvmem (ff3d0000) | rockchip,rv1106-otp | ported (px30_otp_read) | ✅ rockchip-otp0, reads chip id |
| GMAC (ffa80000) | rockchip,rv1106-gmac | ported (dwmac-rk rv1106_ops) | ✅ eth0 Link Up 100M/Full (`gmac/`) |
| GPIO_SYSFS (legacy /sys/class/gpio) | — | mainline (config) | ⬜ goodix script needs it |
| PWM (rockchip) | — | mainline (=m) | ⬜ batch2 =y (backlight) |
| RTC (rv1106-rtc) | — | ported (vendor driver) | ✅ /dev/rtc0 registers + reads |
| USB2 phy (inno, rv1106) | rockchip_usb2phy_* | ported (data, no tuning) | ✅ probes → USB up |
| USB host (DWC3→xhci, ffb00000) | xhci-hcd:usb1 | mainline | ✅ xhci host registered |
| USB OTG gadget (DWC3, eth0) | eth0 | mainline dwc3 | 🔨 host works; eth0 needs dr_mode=peripheral |
| crypto (aes/ccm/ctr/arc4) | modules | mainline | ⬜ batch2 (config =y) |
| PSCI node (removed) | — | — | ✅ deleted (no secure monitor → SMC fault) |
| VOP display (ff990000) | ff990000.vop | ported (rv1126 sibling) | 🔨 binds+DRM+card0; connector WIP |
| PWM backlight (pwm1) | — | mainline (rk3328 fallback) | ✅ backlight up (brightness) |
| RGB666 720×720 panel | — | panel-dpi | 🔨 probes; bus_format + connector WIP |
| GT911 touch (goodix) | goodix, gt911 | mainline | ⬜ M4 (needs GPIO_SYSFS ✅ + node) |
| GPIO_SYSFS / crypto / CFG80211 | — | mainline (config) | ✅ =y (batch2) |
| AIC8800 wifi (bsp/fdrv) | aic8800_* | **out-of-tree** | ✅ M5 — wlan0 up, scanned the site AP at −43dBm (modules, `wifi/VERIFIED-on-c8a3.md`) |
| AIC8800 BT (btlpm) | aic8800_btlpm | **out-of-tree** | 🔨 module built (6.18 vermagic); HCI bring-up not yet exercised |
| NPU (rknpu, ff660000) | rknpu, ff660000.npu | **out-of-tree** | 🔨 M6 built, 0 errors/0 warnings, 99 `rknpu`-prefixed symbols in `System.map`, `&npu {status="okay"}` in the dtb — **not yet flashed/probed on hardware** (build-only session; see `npu/PORT-PROGRESS.md`) |
| RGA 2D (rga2) | rga2 | ported (vendor char-dev) | ✅ /dev/rga, hw 3.3.87975 |
| I2S audio (i2s-tdm) | i2s | rv1126 fallback (=y) | ✅ cpu DAI registers (part of the card below) |
| Audio codec (acodec) | rockchip,rv1106-codec | ported (rv1106_codec.c) | ✅ card `rv1106-acodec`, pcmC0D0p/c (`audio/`); audible test @ bench |
| HPMCU mailbox (ff5c0000) | rockchip,rv1106-mailbox | rk3368 fallback +rv1106 num_chans=1 | ✅ A7<->SCR1 round-trip, 5/5 exact (`mailbox/VERIFIED.md`) |
| PVTM (core+pmu ring-osc) | rockchip,rv1106-*-pvtm | ported (vendor, no mainline) | ✅ both probe; debugfs reads (`pvtm/`) |
| FIQ debugger (ttyFIQ0) | fiq_glue | rockchip | ⬜ optional (we use ttyS2) |

Legend: ✅ verified on hardware · 🔨 built, not yet verified · ⬜ not started.
