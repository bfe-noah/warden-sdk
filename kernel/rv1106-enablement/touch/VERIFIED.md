# GT911 capacitive touch — VERIFIED on warden-c8a3 (2026-08-25)

Touch works on our self-built 6.18.46 (`_b`): the WardenOS UI responds to
taps/swipes (confirmed by hand on the physical panel). Objective evidence:

```
Goodix-TS 3-0014: ID 911, version: 1060
input: Goodix Capacitive TouchScreen as .../i2c-3/3-0014/input/input0
```
`/dev/input/event0` is created and held open by `warden-ui` (pid 517, exclusive
EVIOCGRAB — which is why a second reader sees 0 bytes; the events go to the UI).

## Root cause on `_b`

Touch was dead on `_b` while fine on `_a`. dmesg showed:
```
module goodix: .gnu.linkonce.this_module section size must match the kernel's
built struct module size at run time
```
The **stock rootfs ships `goodix.ko` built for the 5.10 kernel**; it cannot load
on our 6.18 (`struct module` layout mismatch). Our 6.18 `.config` did **not** have
the driver at all (`# CONFIG_TOUCHSCREEN_GOODIX is not set`), and the DTS had no
GT911 node — so nothing drove the GT911.

## Fix

1. **`CONFIG_TOUCHSCREEN_GOODIX=y`** — build the mainline Goodix driver *into* the
   kernel (no module → no vermagic/struct mismatch; the stale rootfs `.ko` still
   fails to insmod but is now harmless).
2. **GT911 DT node** under `&i2c3` (matches the vendor 86-panel wiring):
   ```
   touchscreen@14 {
       compatible = "goodix,gt911";
       reg = <0x14>;
       interrupt-parent = <&gpio0>;
       interrupts = <RK_PA0 IRQ_TYPE_EDGE_FALLING>;
       irq-gpios   = <&gpio0 RK_PA0 GPIO_ACTIVE_HIGH>;
       reset-gpios = <&gpio3 RK_PD0 GPIO_ACTIVE_HIGH>;
   };
   ```
   plus `pinctrl-0 = <&i2c3m2_xfer &tp_rst &tp_irq>` on `&i2c3` and the `tp_rst`
   (GPIO3_D0) / `tp_irq` (GPIO0_A0) pin groups.

Polarity note: mainline goodix drives reset **logical 0 = hold, 1 = release**, and
the GT911 reset is physically active-low, so `reset-gpios` is **ACTIVE_HIGH**
(logical==physical) — NOT the ACTIVE_LOW the vendor 5.10 driver used. Confirmed
against mainline gt911 DT examples (sun7i-a20-wexler-tab7200 etc.): both irq-gpios
and reset-gpios are ACTIVE_HIGH; reg 0x14 needs irq-gpios for address select.

Non-fatal: `Direct firmware load for goodix_911_cfg.bin failed (-2)` — the GT911
uses its flashed internal config; touch works without a cfg.bin.

## Files changed (research/linux-6.18.46)

- `.config` — `CONFIG_TOUCHSCREEN_GOODIX=y`.
- `arch/arm/boot/dts/rockchip/rv1106-warden.dts` — GT911 node on `&i2c3`,
  `tp_rst`/`tp_irq` pin groups under `&pinctrl`.
