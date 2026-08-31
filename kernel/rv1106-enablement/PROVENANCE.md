# Driver provenance & openness ledger

Standing directive: **every** driver we run on the self-built 6.18 kernel must be
open source we can read, harden, and bend (not a binary blob) and this applies
to the drivers *already* ported, not just the new ones. This ledger records, for
each, where the source came from and under what license. Every entry is GPL-2.0
kernel source; nothing here is a binary-only kernel module.

## Legend
- **vendor-src**: C source from the Rockchip 5.10 vendor SDK
  (`flare-edge/sdk/sysdrv/source/kernel/`), forward-ported 5.10 -> 6.18.
- **mainline-sibling**: 6.18 mainline driver, extended with an rv1106 data table
  derived from a sibling SoC (rv1126/px30/rk3562) whose IP block matches.
- All Rockchip vendor kernel code is a GPL-2.0 fork of mainline Linux -> GPL-clean.

## Already-ported drivers (audited for openness per directive)
| Driver | Origin | License | Openness notes |
|---|---|---|---|
| clk-rv1106 (CRU) | vendor-src | GPL-2.0 | full register/PLL source; readable + hardenable |
| pinctrl-rockchip | vendor-src | GPL-2.0-only | rv1106 mux/drive tables in-source |
| rockchip_vop_reg VOP | mainline-sibling (rv1126) | GPL-2.0-only | data table only; no blob |
| phy-rockchip-inno-usb2 | mainline-sibling + vendor regs | GPL-2.0-or-later | rv1106_phy_cfgs from TRM/vendor |
| rockchip_saradc | mainline-sibling (v2) | GPL-2.0-or-later | 2-ch data table |
| rtc-rockchip | vendor-src | GPL-2.0 | whole driver, source |
| rockchip_thermal (tsadc) | vendor-src | GPL-2.0-only | rv1106 code-table + init from source |
| RGA (rga3/) | vendor-src | GPL-2.0 | full multicore 2D engine source |

## In-flight
| Driver | Origin | License | Openness notes |
|---|---|---|---|
| AIC8800 wifi/BT | vendor-src (AICSemi) | `MODULE_LICENSE("GPL")` | full full-MAC source; 50 files use GPL-only kernel/cfg80211/sdio symbols; **no** proprietary/redistribution restriction in-tree |

## The one unavoidable blob, and why it is *not* kernel code
- **`aic8800dc_fw`** (`/oem/usr/ko/`) is device firmware that the GPL host driver
  *uploads to the wifi chip's on-die processor*. It never runs on the A7; it is not
  linked into the kernel. This is the standard Linux firmware split (cf. every
  ath10k/mt76 device) and is the only binary in the wifi path: the driver itself
  is source. No open re-implementation of the AIC on-die MAC firmware exists;
  reverse-engineering it is out of scope for the panel and buys nothing (the host
  driver is where our control + hardening lives).

## NPU: the openness line we will not cross
The **kernel** rknpu driver is portable GPL source (`npu/PORT-PLAN.md`). What is
closed is the *userspace* RKNN runtime + the regcmd stream format, a binary blob.
Per directive we do **not** ship that blob; if NPU compute is ever wanted, the open
path is a from-scratch/reverse-engineered regcmd encoder, tracked separately, never
a vendored binary runtime. (See `npu/PORT-PLAN.md` and the graphics investigation.)

## Posture
Every kernel driver we run is source we hold. The only binaries in the whole
enablement are on-device *firmware* (wifi chip), which by construction cannot be
source on the host. That is the intended openness boundary: **host = source,
device-internal firmware = vendor blob**, and no vendor binary is ever loaded into
*our* kernel's address space.
