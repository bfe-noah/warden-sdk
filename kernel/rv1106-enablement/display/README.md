# M4: display (VOP) port to 6.18

The RV1106 VOP is the older RV-series "lite" VOP (`rockchip,rv1106-vop`, VOP_VERSION
2.0xc), driven by mainline's `rockchip_drm_vop.c` (VOP1), **not** VOP2. Its sibling
**rv1126** is already in mainline 6.18, so the register data is a small reuse, like
the clk/pinctrl ports.

## The driver delta (`rockchip_vop_reg.c`)

`rv1106_vop_data.c.frag`: add this `vop_data` (reuses rv1126's common/modeset/
output/misc/intr/win sub-structs; only the version, the smaller 1280² raster, and
the **`VOP_FEATURE_INTERNAL_RGB` feature** differ), plus the match entry:

```c
{ .compatible = "rockchip,rv1106-vop", .data = &rv1106_vop },
```

`VOP_FEATURE_INTERNAL_RGB` is essential: it is what makes `vop_bind` call
`rockchip_rgb_init()` for the parallel-RGB output (rv1126 routes through MIPI and
omits it).

## The DT (in `../dts/rv1106-warden.dts`)

- `&vop` enabled + **named resets** `resets = <&cru SRST_H_VOP>, <&cru SRST_D_VOP>;
  reset-names = "ahb", "dclk";`, the mainline driver requires these (the vendor
  node omits them; without them: `vop_bind: failed to get ahb reset`).
- `&vop_out_rgb` repointed straight to the panel (mainline has no separate rgb
  node: `rockchip_rgb_init` is a helper the VOP calls).
- `panel` (`panel-dpi`, 720x720, 30 MHz) + `pwm-backlight` on `&pwm1`.

## Verified on warden-c8a3 (2026-08-24)

`CONFIG_DRM_ROCKCHIP=y` + `CONFIG_ROCKCHIP_VOP=y`. On hardware:
```
rockchip-drm display-subsystem: bound ff990000.vop
[drm] Initialized rockchip 1.0.0 for display-subsystem on minor 0
```
`/dev/dri/card0` is present and the **PWM backlight is up** (`/sys/class/backlight`,
brightness settable). **The VOP driver port is validated**: the register data,
version, feature, and resets are right.

## RESOLVED: full UI renders on the panel (2026-08-25)

The connector *and* the deeper black-screen chain that followed it are fixed; the
86-Panel now draws the full WardenOS Dashboard on 6.18 (`_b`), verified by webcam.
The two final root causes were VOP driver bugs: `rgb_dclk_pol` hardcoded inverted,
and the wrong primary scanout window (rv1106 scans out via **WIN1**, not WIN2).
See **`VERIFIED.md`** for the complete bring-up chain, the `_a`-vs-`_b` register
diff that pinned it down, and every file changed.
