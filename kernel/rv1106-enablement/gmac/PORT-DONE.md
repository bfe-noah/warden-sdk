# GMAC (wired 10/100 ethernet) — ✅ VERIFIED on warden-c8a3 (2026-08-25)

**Result: `eth0: Link is Up - 100Mbps/Full - flow control rx/tx`** on our
self-built 6.18.46. The 86-Panel's RMII MAC + on-die 10/100 FEPHY works; a real
link negotiated on the bench.

## What was ported
Mainline `drivers/net/ethernet/stmicro/stmmac/dwmac-rk.c` already supports many
rockchip SoCs (incl. rv1108/rv1126) but not rv1106. Added an `rv1106_ops`
(ported from the vendor 5.10 dwmac-rk.c) + the compatible:

```c
#define RV1106_VOGRF_GMAC_CLK_CON        0x60004   /* in grf syscon@ff000000 (size 0x68000) */
#define RV1106_VOGRF_MACPHY_RMII_MODE    GRF_BIT(0)
#define RV1106_VOGRF_GMAC_CLK_RMII_DIV2  GRF_BIT(2)     /* 100M */
#define RV1106_VOGRF_GMAC_CLK_RMII_DIV20 GRF_CLR_BIT(2) /* 10M */
#define RV1106_VOGRF_MACPHY_CON0         0x60028

rv1106_set_to_rmii()      -> writes VOGRF_GMAC_CLK_CON = RMII_MODE | DIV2
rv1106_set_speed(if,spd)  -> VOGRF_GMAC_CLK_CON = DIV20 (10M) / DIV2 (100M)   [mainline sig: returns int]
rv1106_integrated_phy_powerup/down -> rk_gmac_integrated_fephy_power{up,down}(priv, CON0)

static const struct rk_gmac_ops rv1106_ops = {
	.set_to_rmii = rv1106_set_to_rmii,
	.set_speed = rv1106_set_speed,
	.integrated_phy_powerup = rv1106_integrated_phy_powerup,
	.integrated_phy_powerdown = rv1106_integrated_phy_powerdown,
};
/* OF: { "rockchip,rv1106-gmac", &rv1106_ops } */
```

## Adaptations vs the vendor 5.10 ops
- Mainline `rk_gmac_ops` uses `.set_speed(bsp_priv, interface, speed)` returning
  int (vendor: `.set_rmii_speed(bsp_priv, speed)` void) — adapted.
- Split `.integrated_phy_power(up)` into mainline's `.integrated_phy_powerup` /
  `.integrated_phy_powerdown`, each calling mainline's single-reg
  `rk_gmac_integrated_fephy_power{up,down}(priv, CON0)`.
- **Bandgap trim OMITTED**: the vendor also wrote an OTP-derived bandgap value to
  MACPHY_CON1; that's an analog optimisation the FEPHY runs without, and mainline's
  fephy helper doesn't carry it. Link came up 100M/Full without it. (If signal
  integrity ever needs it, the `bgs` OTP cell now reads — see rng-otp/.)

## DT / config
- `&gmac { status = "okay"; }` — the dtsi `ethernet@ffa80000` node already has
  clocks/resets/`phy-mode="rmii"`/`phy-handle=&rmii_phy` + the integrated
  `ethernet-phy@2` (`phy-is-integrated`). `rockchip,grf=<&grf>` (the big syscon
  covers the 0x60xxx VOGRF offsets). The `bgs`/`txlevel` nvmem-cells are present
  but mainline reads neither, so GMAC does not depend on OTP.
- Kconfig: `STMMAC_ETH`, `STMMAC_PLATFORM`, `DWMAC_ROCKCHIP`, `ROCKCHIP_PHY` (=y).

## Evidence
`rk_gmac-dwmac ffa80000.ethernet eth0: configuring for phy/rmii link mode` →
`eth0: Link is Up - 100Mbps/Full - flow control rx/tx`. PHY bound at
`stmmac-0:02` (integrated). The internal FEPHY reports id 0044.1400; it bound to
the Generic PHY (mainline `net/phy/rockchip.c` INTERNAL_EPHY_ID is 0x1234d400) —
`phy-is-integrated` + c22 was sufficient for a full-duplex 100M link.
