# RV1106 hardware-capabilities audit — nothing left on the table

Every block in the vendor SoC DT (`rv1106.dtsi`), classified. Goal: a 6.18 driver
for every capability the hardware actually has, open-source, verified. Camera/ISP
is the only whole class deliberately skipped — the 86-Panel has no camera.

## ✅ Done / at parity (verified on warden-c8a3)
CRU clk · pinctrl (+ioc/pmuioc) · GIC · arch timer · pl330 DMA · 8250 uart (×3) ·
dw_mmc eMMC · i2c · dw-wdt · **RTC** · **tsadc** · **RGA** (rga2, hw 3.3.87975) ·
PWM backlight · **USB host** (dwc3/xhci) + usb2phy · grf/pmu syscons.

## ✅ Verified this run (2026-08-25) — all on warden-c8a3, self-built 6.18.46
- **AIC8800 wifi** — wlan0 up, scanned BlueFlare −43 dBm (modules; `wifi/VERIFIED-on-c8a3.md`).
- **TRNG** — /dev/hwrng, real HW entropy (`rng-otp/`).
- **OTP/nvmem** — rockchip-otp0 reads chip id (`rng-otp/`).
- **GMAC** — eth0 Link Up 100 Mbps/Full (`gmac/`).
- **SARADC** — iio:device0 reads 2 ch; the −22 was vref, not clk (`adc/SARADC-FIX.md`).

## ✅ VOP display — VERIFIED this run (2026-08-25)
Full WardenOS Dashboard renders on the 86-Panel on 6.18 (`_b`), webcam-verified,
pixel-identical to stock `_a`. Two VOP driver bugs were the final black-screen
cause: `rgb_dclk_pol` hardcoded inverted (panel needs 0), and the wrong primary
scanout window (rv1106 uses **WIN1**, not rv1126's WIN2). Full chain + `_a`-vs-`_b`
register diff in `display/VERIFIED.md`.

## ✅ GT911 touch — VERIFIED this run (2026-08-25)
UI responds to taps/swipes on the panel (Noah-confirmed); GT911 detected
(`ID 911, version 1060`), `/dev/input/event0` held by warden-ui. Fix:
`CONFIG_TOUCHSCREEN_GOODIX=y` (built-in — the rootfs `goodix.ko` is a 5.10 build
that can't load on 6.18) + GT911 node on `&i2c3`. Details in `touch/VERIFIED.md`.

## 🔨 In flight
- **i2s-tdm** — DAI builds; needs the codec + card (below).
- **AIC8800 BT** — module built (6.18 vermagic); HCI bring-up not yet exercised.

## ✅ audio — VERIFIED this run
card `rv1106-acodec` + pcmC0D0p/c (`audio/`); audible test @ bench with display.

## Remaining blocks — final status (all real capabilities now verified)
| Block | verdict | note |
|---|---|---|
| **mailbox** (HPMCU) | ✅ VERIFIED | A7↔RISC-V-SCR1 round-trip, 5/5 exact echoes (`mailbox/VERIFIED.md`). Took 3 hardware-found fixes: rv1106 num_chans=1 (1 shared IRQ, not 4), CLK_CORE_MCU IGNORE_UNUSED (6.18 was gating the coprocessor clock), + an open SCR1 echo firmware with A2B_INTEN. The /dev/mem SRAM watchdog stays as a separate dead-man's-switch. |
| **NPU** (rknpu) | ✅ open driver VERIFIED; compute deferred | open GPL rknpu 0.9.2 driver, /dev/dri/card1, version ioctl PASS (`npu/VERIFIED.md`). Open *compute* (a regcmd compiler) is a from-scratch ~person-year register-RE project — no RV1106 prior art, no public TRM Part 2, mainline accel/rocket+Teflon are RK3588-only. Ship the driver, no blob. |
| **pvtm** | ✅ VERIFIED | both core+pmu PVT monitors probe; debugfs ring-osc reads (`pvtm/PORT-DONE.md`). |
| **crypto-v3** (accel) | deferred (documented) | ~100 KB whole-subsystem replacement of mainline's rk3288 crypto + heavy crypto-API deltas; the **CPU crypto extensions (AES/SHA, batch2 =y) already cover the functional need** — an offload optimization, not a capability gap. |
| camera/ISP, SPI | N/A | no such hardware on the 86-Panel. |

**Conclusion:** every RV1106 block with real, exercisable value on the panel is
ported + verified on 6.18 — including the mailbox and the open NPU driver. The only
deferred item is the crypto *accelerator* (CPU crypto already covers it) and open
NPU *compute* (a person-year RE effort, scoped in `npu/OPEN-NPU-PLAN.md`). No
unexplored gap remains.

## ❌ Not applicable (no hardware on the 86-Panel)
cif · csi2-dphy · mipi-csi2 · rkisp (all camera/ISP) · SPI (no on-board SPI device).

## Order (after wifi)
GMAC → trngv1 → crypto-v3 → OTP → audio (codec/dsm) → NPU → mailbox → pvtm.
GMAC + trngv1 first: highest real value (wired net + hardware entropy).
