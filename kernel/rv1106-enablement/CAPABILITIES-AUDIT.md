# RV1106 hardware-capabilities audit — nothing left on the table

Every block in the vendor SoC DT (`rv1106.dtsi`), classified. Goal: a 6.18 driver
for every capability the hardware actually has, open-source, verified. Camera/ISP
is the only whole class deliberately skipped — the 86-Panel has no camera.

## ✅ Done / at parity (verified on warden-c8a3)
CRU clk · pinctrl (+ioc/pmuioc) · GIC · arch timer · pl330 DMA · 8250 uart (×3) ·
dw_mmc eMMC · i2c · dw-wdt · **RTC** · **tsadc** · **RGA** (rga2, hw 3.3.87975) ·
PWM backlight · **USB host** (dwc3/xhci) + usb2phy · grf/pmu syscons.

## 🔨 In flight
- **AIC8800 wifi/BT** (SDIO on &sdmmc) — the M5 port (subagent building it).
- **VOP display** — binds; connector is tomorrow's on-panel work.
- **i2s-tdm** — DAI builds; needs the codec + card (below).
- **saradc** — driver matches; probe −22 (a clk-rv1106 SARADC divider issue).

## ⬜ Remaining capabilities to port (open vendor source exists for all)
| Block | vendor compat | 6.18 mainline? | value | plan |
|---|---|---|---|---|
| **GMAC** (wired ethernet) | dwmac, `&gmac` okay on-board | no rv1106 in dwmac-rockchip | HIGH (wired net) | add rv1106 glue to dwmac-rockchip + PHY/DT |
| **crypto-v3** (accelerator) | rockchip,crypto-v3 | mainline has only v1 (rk3288) | med | port vendor crypto-v3 driver |
| **trngv1** (hardware RNG) | rockchip,trngv1 | none | HIGH (entropy for keys) | port vendor trng → hwrng |
| **OTP/nvmem** | rockchip,rv1106-otp | no rv1106 in rockchip-otp | med (chip id / MAC) | add rv1106 to rockchip-otp |
| **mailbox** (HPMCU) | rockchip,rv1106-mailbox | only rk3368 | med (proper coproc mbox vs /dev/mem) | port |
| **NPU** (rknpu) | rockchip,rknpu | none (no open userspace) | low-med | port kernel driver (`npu/PORT-PLAN.md`) |
| **audio codec + DSM** | rv1106-codec, codec-digital | none | med (panel has speaker) | port rv1106_codec.c + rk_dsm.c + card |
| **pvtm** (PVT monitors) | rv1106-{core,pmu}-pvtm | limited | low (DVFS) | port if DVFS pursued |

## ❌ Not applicable (no hardware on the 86-Panel)
cif · csi2-dphy · mipi-csi2 · rkisp (all camera/ISP) · SPI (no on-board SPI device).

## Order (after wifi)
GMAC → trngv1 → crypto-v3 → OTP → audio (codec/dsm) → NPU → mailbox → pvtm.
GMAC + trngv1 first: highest real value (wired net + hardware entropy).
