# ADR 0001 — Kernel Forward-Port

**Status:** Accepted (2026-08-25). Supersedes the README's original plan44/6.6 goal.

## Context
The vendor kernel is Rockchip 5.10.160 (via Luckfox). We want the newest stable
Linux runnable on our Buildroot LTS (2025.02.x). Two candidate paths existed:
(a) baseline on plan44's OpenWrt RV1106 fork (Linux 6.6, 152 RV1106 patches), or
(b) forward-port the vendor tree directly onto a chosen upstream stable.

## Decision
Forward-port **directly to Linux 6.18.46** from vendor 5.10.160, carrying **no
plan44/OpenWrt code**. Reuse the already-upstream **rv1126** register data where it
matches the RV1106 ("RV-series lite" VOP, clk, pinctrl), and carry the RV1106
deltas as a reviewable patch series in `patches/`.

## Consequences
- Done and **hardware-verified on `warden-c8a3`**: clk, pinctrl, eMMC, GMAC, TRNG,
  OTP, SARADC/TSADC, RTC, USB host, PWM/backlight, VOP display, GT911 touch, AIC8800
  wifi, RGA, I2S audio, HPMCU mailbox, open NPU driver, PVTM.
- **uClibc stays load-bearing** — RGA/MPP/ISP/NPU userspace ship as uClibc-only
  blobs; a glibc swap breaks media (flare-edge #51, wontfix). Any kernel bump
  inherits this.
- Kernel bumps risk struct-ABI breaks for out-of-tree modules (the AIC8800/VLAN
  `struct net_device` offset saga). Mitigation: ship one matched full boot+oem
  image, never a partial reflash.
- Deferred by design: crypto-v3 accelerator (CPU crypto already covers the need),
  open NPU *compute* (a person-year register-RE effort, no RV1106 prior art).
