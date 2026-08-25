# SPDX-License-Identifier: GPL-2.0-only
#
# aic8800_btlpm — Bluetooth low-power-mode / HCI wake companion module,
# ported in-tree from the vendor out-of-tree Makefile
# (flare-edge/sdk/sysdrv/drv_ko/wifi/aic8800dc/aic8800_btlpm/Makefile).
# lpm.c stays out of the build (CONFIG_SUPPORT_LPM was already off in the
# vendor default; aic8800_btlpm.c's own has_lpm_enabled/lpm_fops logic is
# self-contained and does not call into lpm.c).

ccflags-y += -DCONFIG_PLATFORM_ROCKCHIP
ccflags-y += -I$(srctree)/$(src)/../aic8800_bsp

obj-$(CONFIG_AIC8800_BTLPM_SUPPORT) += aic8800_btlpm.o
aic8800_btlpm-y := \
	aic_bluetooth_main.o \
	rfkill.o
