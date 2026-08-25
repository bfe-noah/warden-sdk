# TRNG + OTP port (batch A) — 6.18

## TRNG (hardware RNG) — ✅ VERIFIED on warden-c8a3 (2026-08-25)
rv1106's `rockchip,trngv1` is the same standalone TRNG_V1 IP as rk3588 (identical
register map). Mainline `drivers/char/hw_random/rockchip-rng.c` already drives it.

**Delta:** one OF-table entry reusing `rk3588_soc_data`:
```c
{ .compatible = "rockchip,rv1106-rng", .data = (void *)&rk3588_soc_data },
```
(The driver grabs clocks via `clk_bulk_get_all` and treats reset as optional, so
rv1106's clock/reset names need no special handling.) Kconfig
`HW_RANDOM_ROCKCHIP=y` (already set).

**DT** (`rv1106-warden.dts`): override the dtsi `rng@ff448000` node —
```dts
&rng { compatible = "rockchip,rv1106-rng"; status = "okay"; };
```

**Evidence:** `/dev/hwrng` present, `rng_current = rockchip-rng`,
`dd if=/dev/hwrng bs=16` → `c697 503d f9db 6b84 50e4 e1ee f232 b2ae` (real HW
entropy, non-zero). Hardware entropy source for the panel's crypto/keys.

## OTP / nvmem — ✅ VERIFIED on warden-c8a3 (2026-08-25)
Reads real data: `dd .../rockchip-otp0/nvmem bs=1 count=16 | xxd` →
`5211 02fe 084d 5231 0000 0000 3b15 0000` (contains "MR1" chip id) — no timeout.
Mainline `drivers/nvmem/rockchip-otp.c` gains an `rv1106_data` + compatible.

**Delta:**
```c
static const char * const rv1106_otp_clocks[] = {
	"usr", "sbpi", "apb", "phy", "arb", "pmc",   /* matches DT clock-names */
};
static const struct rockchip_data rv1106_data = {
	.size = 0x80,
	.clks = rv1106_otp_clocks,
	.num_clks = ARRAY_SIZE(rv1106_otp_clocks),
	.reg_read = px30_otp_read,     /* user-mode OTPC_USER interface */
};
/* OF: { "rockchip,rv1106-otp", &rv1106_data } */
```
Kconfig `NVMEM_ROCKCHIP_OTP=y`. **DT:** `&otp { status = "okay"; };` (the dtsi
`otp@ff3d0000` node already carries compatible + the 6 clocks/resets).

**First try** used `.reg_read = rk3588_otp_read` → `timeout during read setup`
(rk3588 uses a different addressing path). Corrected to `px30_otp_read` — the
mainline user-mode OTPC_USER read, the same sequence the vendor 5.10 driver used
for rv1106 (its `rk3568_otp_read`). Confirmed: cell reads return real data.
