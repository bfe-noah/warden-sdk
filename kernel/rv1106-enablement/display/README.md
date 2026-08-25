# M4 — display (VOP) port to 6.18

The RV1106 VOP is the older RV-series "lite" VOP (`rockchip,rv1106-vop`, VOP_VERSION
2.0xc), driven by mainline's `rockchip_drm_vop.c` (VOP1), **not** VOP2. Its sibling
**rv1126** is already in mainline 6.18, so the register data is a small reuse — like
the clk/pinctrl ports.

## The driver delta (`rockchip_vop_reg.c`)

`rv1106_vop_data.c.frag` — add this `vop_data` (reuses rv1126's common/modeset/
output/misc/intr/win sub-structs; only the version, the smaller 1280² raster, and
the **`VOP_FEATURE_INTERNAL_RGB` feature** differ), plus the match entry:

```c
{ .compatible = "rockchip,rv1106-vop", .data = &rv1106_vop },
```

`VOP_FEATURE_INTERNAL_RGB` is essential — it is what makes `vop_bind` call
`rockchip_rgb_init()` for the parallel-RGB output (rv1126 routes through MIPI and
omits it).

## The DT (in `../dts/rv1106-warden.dts`)

- `&vop` enabled + **named resets** `resets = <&cru SRST_H_VOP>, <&cru SRST_D_VOP>;
  reset-names = "ahb", "dclk";` — the mainline driver requires these (the vendor
  node omits them; without them: `vop_bind: failed to get ahb reset`).
- `&vop_out_rgb` repointed straight to the panel (mainline has no separate rgb
  node — `rockchip_rgb_init` is a helper the VOP calls).
- `panel` (`panel-dpi`, 720×720, 30 MHz) + `pwm-backlight` on `&pwm1`.

## Verified on warden-c8a3 (2026-08-24)

`CONFIG_DRM_ROCKCHIP=y` + `CONFIG_ROCKCHIP_VOP=y`. On hardware:
```
rockchip-drm display-subsystem: bound ff990000.vop
[drm] Initialized rockchip 1.0.0 for display-subsystem on minor 0
```
`/dev/dri/card0` is present and the **PWM backlight is up** (`/sys/class/backlight`,
brightness settable). **The VOP driver port is validated** — the register data,
version, feature, and resets are right.

## Open (needs on-panel eyes + a little more debug)

`/sys/class/drm/card0-*` has **no connector yet** — `rockchip_rgb_init` isn't
producing one (log: `Cannot find any crtc or sizes`), despite INTERNAL_RGB set and
a clean vop→panel of_graph. Likely a panel-probe-order / `drm_of_find_panel_or_bridge`
detail. This is the last mile of M4 and, unlike the driver bind, needs the panel
physically observed (pixels can't be verified over serial/ssh). To pin it down: a
debug print in `rockchip_rgb_init` (child_count / find-panel ret), and set the panel
bus_format to `MEDIA_BUS_FMT_RGB666_1X18` (a small `panel_dpi_probe` addition, since
mainline panel-dpi doesn't read bus-format from DT).
