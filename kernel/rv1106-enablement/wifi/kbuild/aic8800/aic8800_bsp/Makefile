# SPDX-License-Identifier: GPL-2.0-only
#
# aic8800_bsp: SDIO bus glue + firmware bootstrap, ported in-tree from the
# vendor out-of-tree Makefile (flare-edge/sdk/sysdrv/drv_ko/wifi/aic8800dc/
# aic8800_bsp/Makefile). CONFIG_AIC_FW_PATH is now a real Kconfig string
# symbol (see ../Kconfig) so it needs no -D here; the driver picks it up
# from include/generated/autoconf.h directly (aicsdio.c, aic_bsp_driver.h).
#
# Vendor CONFIG_* build-time knobs below are preserved as fixed -D flags
# (not full Kconfig symbols) since this port only needs the one build
# configuration this board actually uses (SDIO, Rockchip, no USB/PCI/mesh).

ccflags-y += -DAICWF_SDIO_SUPPORT
ccflags-y += -DCONFIG_SDIO_PWRCTRL
ccflags-y += -DCONFIG_PLATFORM_ROCKCHIP
ccflags-y += -DCONFIG_LINK_DET_5G
ccflags-y += -DCONFIG_VRF_DCDC_MODE
ccflags-y += -DCONFIG_PREALLOC_TXQ
ccflags-y += -DCONFIG_DPD -DCONFIG_FORCE_DPD_CALIB
ccflags-y += -DCONFIG_RESV_MEM_SUPPORT
ccflags-y += -DCONFIG_AMSDU_RX
ccflags-y += -Wno-unused-function

obj-$(CONFIG_AIC_WLAN_SUPPORT) += aic8800_bsp.o
aic8800_bsp-y := \
	aic8800dc_compat.o \
	aic8800d80_compat.o \
	aic_bsp_main.o \
	aic_bsp_driver.o \
	aicsdio.o \
	aicsdio_txrxif.o \
	md5.o \
	aicwf_txq_prealloc.o
