# RV1106 hardware-capabilities audit — nothing left on the table

Every block in the vendor SoC DT (`rv1106.dtsi`), classified. Goal: a 6.18 driver
for every capability the hardware actually has, open-source, verified. Camera/ISP
is the only whole class deliberately skipped — the 86-Panel has no camera.

## ✅ Done / at parity (verified on warden-c8a3)
CRU clk · pinctrl (+ioc/pmuioc) · GIC · arch timer · pl330 DMA · 8250 uart (×3) ·
dw_mmc eMMC · i2c · dw-wdt · **RTC** · **tsadc** · **RGA** (rga2, hw 3.3.87975) ·
PWM backlight · **USB host** (dwc3/xhci) + usb2phy · grf/pmu syscons.

## ✅ Verified this run (2026-08-25) — all on warden-c8a3, self-built 6.18.46
- **AIC8800 wifi** — wlan0 up, scanned the site AP at −43 dBm (modules; `wifi/VERIFIED-on-c8a3.md`).
- **TRNG** — /dev/hwrng, real HW entropy (`rng-otp/`).
- **OTP/nvmem** — rockchip-otp0 reads chip id (`rng-otp/`).
- **GMAC** — eth0 Link Up 100 Mbps/Full (`gmac/`).
- **SARADC** — iio:device0 reads 2 ch; the −22 was vref, not clk (`adc/SARADC-FIX.md`).

## 🔨 In flight
- **VOP display** — binds; connector is tomorrow's on-panel work.
- **i2s-tdm** — DAI builds; needs the codec + card (below).
- **AIC8800 BT** — module built (6.18 vermagic); HCI bring-up not yet exercised.

## ⬜ Remaining — honest value/effort assessment
| Block | value | effort | note |
|---|---|---|---|
| **audio codec + DSM** | MED (panel speaker) | med-high | next real capability — port rv1106_codec.c (2317L) + rk_dsm.c + simple-audio-card; ASoC 5.10→6.18 deltas. Card-registers verifiable now; audible test joins tomorrow's bench session |
| **mailbox** (HPMCU) | MED | small | proper coproc mbox; the /dev/mem R5 path already works, so this is a cleanup not a gap |
| **crypto-v3** (accel) | LOW-incremental | HIGH | ~100KB whole-subsystem replacement of mainline's rk3288 crypto + heavy crypto-API deltas. **CPU crypto extensions (AES/SHA, batch2 =y) already cover the functional need** — this is an offload optimization, not a capability gap. Defer to a dedicated session |
| **NPU** (rknpu) | LOW (open) | med | kernel driver ports, but NO open userspace regcmd runtime exists — the driver would register with nothing able to submit jobs openly. Not worth shipping without an open encoder (`npu/PORT-PLAN.md`) |
| **pvtm** | LOW | small | DVFS monitors; only if DVFS is pursued |

## ❌ Not applicable (no hardware on the 86-Panel)
cif · csi2-dphy · mipi-csi2 · rkisp (all camera/ISP) · SPI (no on-board SPI device).

## Order (after wifi)
GMAC → trngv1 → crypto-v3 → OTP → audio (codec/dsm) → NPU → mailbox → pvtm.
GMAC + trngv1 first: highest real value (wired net + hardware entropy).
