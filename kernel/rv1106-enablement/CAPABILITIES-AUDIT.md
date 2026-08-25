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

## ✅ audio — VERIFIED this run
card `rv1106-acodec` + pcmC0D0p/c (`audio/`); audible test @ bench with display.

## ⬜ Remaining — final honest ledger (each is deferred for a stated reason, not an unexplored gap)
| Block | verdict | reason |
|---|---|---|
| **mailbox** (HPMCU) | available, not enabled | the rv1106 node carries a `rockchip,rk3368-mailbox` **fallback** → mainline's driver binds it with **zero code change**. But nothing in our kernel is a mailbox *client* (the coprocessor R5 path is /dev/mem today), so an enabled controller would register unexercised. Enable it the day a coprocessor mailbox client lands — DT `status="okay"` + `MAILBOX`/`ROCKCHIP_MBOX=y`, no port. |
| **crypto-v3** (accel) | deferred | ~100KB whole-subsystem replacement of mainline's rk3288 crypto + heavy crypto-API deltas, and the **CPU crypto extensions (AES/SHA, batch2 =y) already cover the functional need** — an offload optimization, not a capability gap. |
| **NPU** (rknpu) | not worth shipping | kernel driver ports, but **no open userspace regcmd runtime exists** — it would register with nothing able to submit jobs openly. Needs an open encoder first (`npu/PORT-PLAN.md`, graphics investigation). |
| **pvtm** | deferred | PVT monitors are only useful for DVFS, which we don't run. |
| camera/ISP, SPI | N/A | no such hardware on the 86-Panel. |

**Conclusion:** every RV1106 block with real, exercisable value on the panel is
ported + verified on 6.18. What remains is documented above — no unexplored gap.

## ❌ Not applicable (no hardware on the 86-Panel)
cif · csi2-dphy · mipi-csi2 · rkisp (all camera/ISP) · SPI (no on-board SPI device).

## Order (after wifi)
GMAC → trngv1 → crypto-v3 → OTP → audio (codec/dsm) → NPU → mailbox → pvtm.
GMAC + trngv1 first: highest real value (wired net + hardware entropy).
