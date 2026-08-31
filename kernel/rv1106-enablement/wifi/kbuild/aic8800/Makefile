# SPDX-License-Identifier: GPL-2.0-only
#
# Link order matters: bsp before fdrv before btlpm, mirroring the working
# insmod order (insmod_wifi.sh) now that these are built-in (=y) instead of
# modules, see PORT-PLAN.md section 4.5.
obj-$(CONFIG_AIC_WLAN_SUPPORT)      += aic8800_bsp/
obj-$(CONFIG_AIC8800_WLAN_SUPPORT)  += aic8800_fdrv/
obj-$(CONFIG_AIC8800_BTLPM_SUPPORT) += aic8800_btlpm/
