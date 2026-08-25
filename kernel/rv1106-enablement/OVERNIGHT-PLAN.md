# Overnight autonomous kernel-completion workflow

**Directive (Noah, 2026-08-24 night):** port/enable **every** remaining RV1106
hardware capability on the self-built 6.18 kernel, **open-source-first** — we want
*source* we can read, harden, and bend to our needs (vendor SDK source, upstream,
community repos, or reverse-engineering), never binary blobs. Not just ports —
**new drivers if necessary**. Leave zero hardware capability or customization on
the table. Fully autonomous, runs overnight; the display's last mile is deferred
to tomorrow morning (needs eyes on the panel).

## Open-source sourcing policy (in order of preference)
1. **Vendor SDK source** — `flare-edge/sdk/sysdrv/source/kernel/drivers/…`. Every
   driver below ships as C source there (aic8800, rknpu, rga, rtc, tsadc, i2s all
   have source — the *only* closed pieces are userspace runtimes like RKNN, not
   the kernel drivers). Port 5.10 → 6.18.
2. **Mainline / sibling** — reuse rv1126/px30 data where the IP matches (clk,
   pinctrl, vop, saradc, usb2phy all used this).
3. **Community open drivers** — deep web research (AICSemi upstream/github, the
   NPU reverse-engineering efforts, etc.) for a cleaner or better-understood base.
4. **Reverse-engineering** — register maps from the TRM + the vendor source, as a
   last resort or to harden.
For each: understand it, prefer the most upstream-aligned source, harden it, and
add a regression check where feasible.

## Targets (priority order)

| # | Driver | Source | Verify (self, no visual) |
|---|---|---|---|
| 1 | **AIC8800 wifi + BT** (SDIO full-MAC) | SDK aic8800_{bsp,fdrv,btlpm} + AICSemi github | `wlan0` up, scan, connect |
| 2 | **RGA 2D** (multicore rga2) | SDK + mainline rockchip/rga | `/dev/rga`, blit test |
| 3 | **rknpu NPU** (kernel driver) | SDK rknpu + RE efforts | `/dev/rknpu`, submit a job |
| 4 | **RTC** (rv1106-rtc) | SDK drivers/rtc | `/dev/rtc0`, hwclock |
| 5 | **tsadc thermal** | SDK data port | `thermal_zone0/temp` |
| 6 | **i2s-tdm audio** | rv1126 fallback + config | sound card / aplay |
| 7 | **saradc -22 fix** | clk-rate debug | IIO device reads |
| 8 | **eth0 USB gadget** | dr_mode + configfs | `eth0` to xps |
| 9 | **Capabilities audit** | TRM + SDK sweep | enable/port everything else |

### Capabilities audit (#9) — nothing left on the table
Enumerate every RV1106 block and confirm a 6.18 driver: crypto accelerator + TRNG,
mailbox/HPMCU integration, DSMC/flexbus, SFC/SPI-NOR, remaining SPI, CAN, PWM
(fan/other), DMA2, DDR monitor, the second/other UARTs, GMAC (if wired), pvtm,
otp/nvmem, dsmc. Anything with hardware present and no driver → port or write it.

## Execution model
Pipeline: research (Sonnet subagents, in background) ∥ port (me) → build → flash to
c8a3 slot _b → verify on hardware → commit to warden-sdk. Serial bottleneck is
c8a3; research/porting overlaps verification. **Resilience:** krecover is hardened
(serial-primary + Pi/rkdeveloptool bootcount-reset fallback); on a port that won't
land, capture the state, mark it in DRIVER-PARITY.md, and move on — never leave
c8a3 stuck, never block the whole run on one driver. Commit after every landed
driver so progress is durable.
