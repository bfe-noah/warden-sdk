# Display (VOP + RGB panel): VERIFIED on warden-c8a3 (2026-08-25)

The 86-Panel renders the **full WardenOS Dashboard UI** on our self-built Linux
6.18.46 (`_b` slot), pixel-identical to the stock 5.10 `_a` slot. Verified by
webcam pointed at the physical panel (pixels can't be checked over serial/ssh);
`warden-ui -b DRM` drawing the Dashboard: nav sidebar, "Wellhead 03" site card,
Mesh/WAN/LAN/RS485 status rows, correct colors, no banding, no colour swap.

This closes the "Open" item in `README.md` (connector-not-created + the deeper
black-screen chain that followed it).

## The bring-up chain (each step was a real blocker)

1. **Connector never created** (`Cannot find any crtc`): `CONFIG_ROCKCHIP_RGB`
   was not set, AND the vendor `&rgb` bridge node's dangling endpoint
   (`rgb_in_vop -> vop_out_rgb`) read to fw_devlink as a vop<->panel dependency
   cycle, so the VOP probed before the panel. Fix: enable `ROCKCHIP_RGB` +
   `/delete-node/ &rgb;` so `vop_out_rgb <-> panel_in_vop` is the only graph link
   (rockchip_rgb_init then defers + retries and finds the panel -> `LVDS-1`).
2. **Backlight dark**: `&pwm1` had `pinctrl-names = "active"`; mainline
   pwm-rockchip relies on the driver core auto-applying the **`"default"`** state,
   so the PWM pin was never muxed. Fix: rename to `"default"`.
3. **bus-format unset** (RGB output width undefined): mainline `panel-dpi`
   ignores the DT `bus-format`. Fix: small `panel_dpi_probe()` patch to read it +
   `bus-format = <MEDIA_BUS_FMT_RGB666_1X18>` on the panel node.
4. **RGB output pins unmuxed** (no data reaches the panel): the 22-pin parallel
   bus (`lcd_clk` + `lcd_d0..d17` + den/hsync/vsync) needs the `&lcd_pins` mux.
   The vendor carried it as `pinctrl-0 = <&lcd_pins>` **on the `&rgb` node**, which
   we deleted in step 1. Fix: re-attach it on `&vop` (`pinctrl-names="default"`;
   the core auto-applies it at VOP probe).
5. **`reset-gpios` on the panel node is WRONG** (leave it off): GPIO0_A1 resets the
   CH32V003 panel-init MCU, released once early by U-Boot `board_init()`. Handing
   it to `panel-simple` makes `drm_panel_prepare()` re-reset the MCU mid-scanout.
   Luckfox deleted these props upstream (commit e2b0ffa22); the flare-edge 5.10 DTS
   documents the deletion as "the fix." We match: **backlight only, no reset-gpios**.

## The two root causes of the final "backlit-black" (VOP driver bugs)

With connector + backlight + pins + panel-init all correct, the screen was still
physically black (backlight on, `LVDS-1` connected, VOP streaming, fb0 720x720,
`modetest`/splash written): the classic "everything healthy, glass dark" state.
Found by dumping the VOP register block `0xff990000` on the **working `_a` slot**
and diffing against `_b` (same SoC, same register map):

**Bug 1: DCLK polarity inverted.** `rockchip_drm_vop.c` hardcodes
`rgb_dclk_pol = 1` for the LVDS/RGB output. The panel latches pixel data on the
**non-inverted** edge: `_a` reads `PX30_DSP_CTRL0` (0xff990020) = `0x1`
(rgb_dclk_pol bit1 = **0**). The vendor 5.10 derives it as
`(bus_flags & PIXDATA_DRIVE_NEGEDGE) ? 1 : 0`, which is 0 for this panel. With the
inverted clock the panel samples RGB on the wrong edge -> black. **Fix:** set
`rgb_dclk_pol` to 0 in the `DRM_MODE_CONNECTOR_LVDS` case.

**Bug 2: wrong primary scanout window.** The rv1106 VOP scans out through
**WIN1**, but our port reused rv1126's win table (`win0`-overlay + **`win2`**-primary).
The vendor `rv1106_vop_win_data` is `{ NULL-win0, rk3366_lit_win1_data-primary }`,
i.e. WIN1 (`PX30_WIN1_*` at 0x090). Mainline was configuring WIN2, which never
reaches this SoC's RGB interface, so WIN1 stayed all-zero and nothing scanned out.
Register evidence (`_a` working vs `_b` broken, before the fix):

| reg | name | `_a` (works) | `_b` (black) |
|---|---|---|---|
| 0x090 | WIN1_CTRL0 (enable) | `0x00000001` | `0x00000000` |
| 0x098 | WIN1_VIR (stride)   | `0x000002D0` (720) | `0x00000000` |
| 0x0a0 | WIN1_MST (fb addr)  | `0x0F900000` | `0x00000000` |
| 0x0a4 | WIN1_DSP_INFO (size)| `0x02CF02CF` (720x720) | `0x00EF013F` (stale) |

**Fix:** `rv1106_vop_win_data[] = { { .phy = &px30_win1_data, PRIMARY } }`
(`px30_win1_data` already exists in mainline over `PX30_WIN1_*`, just wasn't wired
for rv1106). After the fix `_b` reads WIN1_CTRL0=1, WIN1_DSP_INFO=0x02CF02CF:
matching `_a`.

## Files changed (in research/linux-6.18.46)

- `drivers/gpu/drm/rockchip/rockchip_drm_vop.c`: `rgb_dclk_pol` 1 -> 0 (LVDS case).
- `drivers/gpu/drm/rockchip/rockchip_vop_reg.c`: `rv1106_vop_win_data` uses
  `px30_win1_data` as the single PRIMARY window (was rv1126's win0+win2).
- `drivers/gpu/drm/panel/panel-simple.c`: `panel_dpi_probe()` reads DT `bus-format`.
- `arch/arm/boot/dts/rockchip/rv1106-warden.dts`: `&vop` pinctrl `<&lcd_pins>`;
  `&pwm1` pinctrl "default"; `/delete-node/ &rgb`; panel `bus-format` RGB666,
  no reset-gpios.

## Known separate issue (NOT display)

`_b` still cold-reboots periodically (pre-existing `_b` general-stability issue,
independent of the display: the panel renders correctly the whole time it is up).
Tracked separately; the high-bootlimit env (`bootlimit=10000`) keeps these reboots
from tripping `altbootcmd=download` into the loader during bring-up.
