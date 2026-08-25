# Remaining ports — concrete execution steps (scoped from source)

Scoped against mainline 6.18 + vendor 5.10 source while the wifi transplant builds.
Execute in this order once the tree is free (after wifi lands + verifies). Each:
port in tree → build → flash c8a3 _b → verify (self, no visual) → commit.

## 1. TRNG (hardware RNG) — TRIVIAL (~2 lines), HIGH value
**Finding:** rv1106 `rockchip,trngv1` registers (CTRL 0x0000, STAT 0x0004, MODE
0x0008, ISTAT 0x0014, RAND0–7 0x0020–0x3C) are **byte-identical** to rk3588, which
mainline `hw_random/rockchip-rng.c` already drives via `rk3588_rng_read` +
`rk3588_soc_data`.
**Port:** add `{ .compatible = "rockchip,rv1106-rng", .data = &rk3588_soc_data }`
to the OF table; give the DT node `compatible = "rockchip,rv1106-rng",
"rockchip,rk3588-rng"`. Confirm `rk3588_soc_data` clock/reset names match rv1106 DT
(`clocks`/`resets`); adjust soc_data if not. Kconfig `HW_RANDOM_ROCKCHIP=y`.
**Verify:** `/dev/hwrng` present; `dd if=/dev/hwrng bs=32 count=1 | xxd` non-zero;
`cat /sys/class/misc/hw_random/rng_current` = rockchip.

## 2. GMAC (wired ethernet) — MEDIUM, HIGH value
**Finding:** node `ethernet@ffa80000` = `snps,dwmac-4.20a` + `rockchip,rv1106-gmac`,
**phy-mode rmii**, `phy-handle=&rmii_phy` (ethernet-phy@2). The **integrated EPHY
(0x1234d400) is ALREADY in mainline** `net/phy/rockchip.c` → no PHY port needed.
Bandgap trim `macphy_bgs` comes from OTP (soft dep — PHY works without it, slightly
worse analog). Vendor glue lives in `dwmac-rk.c`; mainline is `dwmac-rockchip.c`.
**Port:** add an rv1106 `struct rockchip_gmac_ops` to mainline `dwmac-rockchip.c`
(GRF bits for RMII mode + `set_to_rmii`/`set_rmii_speed` from vendor `dwmac-rk.c`
rv1106 section) + the compatible; wire the board `&gmac` node (clocks/resets/PHY as
above). Kconfig: `STMMAC_ETH=y`, `DWMAC_ROCKCHIP=y`, `ROCKCHIP_PHY=y`.
**Verify:** `ip link` shows the MAC; `ethtool eth?`; link-up on a patch cable if the
bench has one, else MDIO read of PHY ID via the driver's probe log.

## 3. OTP / nvmem — SMALL, MEDIUM value (also feeds GMAC bandgap + chip-id)
**Finding:** vendor `nvmem/rockchip-otp.c` has `rv1106_otp_clocks[]` + an rv1106
read path; mainline has px30/rk3308/rk3576/rk3588 but no rv1106.
**Port:** add `rv1106_data` (reg_read + clocks/offset/size) from vendor to mainline
`rockchip-otp.c` + compatible `rockchip,rv1106-otp`. Kconfig `NVMEM_ROCKCHIP_OTP=y`.
**Verify:** nvmem device in `/sys/bus/nvmem/devices/`; read chip-id cell.

## 4. Audio (codec + DSM + card) — MEDIUM, MEDIUM value
i2s-tdm DAI already builds (rv1126 fallback). Port vendor `rv1106_codec.c` (acodec)
+ `rk_dsm.c` (digital speaker) + a `simple-audio-card`/`rockchip,rv1106-codec` node.
**Verify:** `aplay -l` shows a card; `speaker-test` writes without error (audible
check is tomorrow's on-panel item).

## 5. crypto-v3 (accelerator) — LARGER, MEDIUM value
Vendor `crypto/rockchip/rk_crypto_v3*.c` (v3 core + ahash + skcipher); mainline has
only v1 (rk3288). Port the v3 files + `rockchip,crypto-v3` compatible. Larger
surface; do after the quick wins. **Verify:** `/proc/crypto` lists rk hw entries;
`tcrypt` or an openssl-engine smoke test.

## 6. NPU kernel driver — MEDIUM, LOW-MED value (`npu/PORT-PLAN.md`)
Kernel driver only; no open userspace regcmd encoder (openness line — no blob).

## 7. mailbox (HPMCU) — SMALL, MED value — proper coproc mbox vs /dev/mem hack.
## 8. pvtm — SMALL, LOW value — DVFS monitors; only if DVFS is pursued.

Order rationale: TRNG (near-free, high value) → GMAC (high value, PHY already
mainline) → OTP (small, feeds GMAC) → audio → crypto → NPU → mailbox → pvtm.
