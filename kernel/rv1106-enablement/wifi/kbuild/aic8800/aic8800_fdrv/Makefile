# SPDX-License-Identifier: GPL-2.0-only
#
# aic8800_fdrv — the cfg80211 full-MAC driver ("fdrv"), ported in-tree from
# the vendor out-of-tree Makefile (flare-edge/sdk/sysdrv/drv_ko/wifi/
# aic8800dc/aic8800_fdrv/Makefile). SDIO-only build (this board has no
# USB/PCI variant) — usb_host.c and rwnx_pci.c are dropped from the tree
# entirely (PORT-PLAN.md §4.1 flagged both as candidates to drop "unless a
# build error proves them referenced elsewhere" — that held for these two).
# rwnx_mesh.c was ALSO dropped initially on the same theory but had to be
# restored: rwnx_tx.c's NL80211_IFTYPE_MESH_POINT switch-case is not
# preprocessor-gated, so rwnx_mesh.h's types/prototypes are needed to
# compile even though this board never creates a mesh-type interface.
# md5.o is dropped from THIS module's object list only: it is byte-identical
# to aic8800_bsp's
# md5.c (same non-static MD5Init/MD5Update/... symbols) and, unlike the
# old separate .ko world, both drivers now link into one vmlinux — keeping
# both objects would be a multiple-definition link error. aic8800_bsp
# links first (../Makefile) so fdrv's calls resolve against its md5.o.

ccflags-y += -DCONFIG_RWNX_FULLMAC
ccflags-y += -DAICWF_SDIO_SUPPORT
ccflags-y += -DCONFIG_SDIO_PWRCTRL
ccflags-y += -DCONFIG_PLATFORM_ROCKCHIP
ccflags-y += -DCONFIG_START_FROM_BOOTROM
ccflags-y += -DCONFIG_PMIC_SETTING
ccflags-y += -DCONFIG_VRF_DCDC_MODE
ccflags-y += -DCONFIG_ROM_PATCH_EN
ccflags-y += -DCONFIG_RWNX_RADAR
ccflags-y += -DCONFIG_RWNX_BCMC
ccflags-y += -DNX_TXQ_CNT=5
ccflags-y += -DCONFIG_USER_MAX=1
ccflags-y += -DCONFIG_RWNX_DBG
ccflags-y += -DUSE_5G
ccflags-y += -DCONFIG_TXRX_THREAD_PRIO
ccflags-y += -DCONFIG_SUPPORT_REALTIME_CHANGE_MAC
ccflags-y += -DCONFIG_PREALLOC_TXQ
ccflags-y += -DCONFIG_DPD -DCONFIG_FORCE_DPD_CALIB
ccflags-y += -DCONFIG_FILTER_TCP_ACK
ccflags-y += -DCONFIG_RESV_MEM_SUPPORT
ccflags-y += -DCONFIG_SHUTDOWN_CALLBACK
ccflags-y += -DCONFIG_MCC
ccflags-y += -DCONFIG_RFTEST
ccflags-y += -DCONFIG_RX_NETIF_RECV_SKB
ccflags-y += -DAICWF_RX_REORDER
ccflags-y += -DAICWF_ARP_OFFLOAD

# FW/IPC sizing (vendor 8800DCDW u02 variant)
ccflags-y += -DNX_VIRT_DEV_MAX=4
ccflags-y += -DNX_REMOTE_STA_MAX_FOR_OLD_IC=10
ccflags-y += -DNX_REMOTE_STA_MAX=32
ccflags-y += -DNX_MU_GROUP_MAX=62
ccflags-y += -DNX_TXDESC_CNT=64
ccflags-y += -DNX_TX_MAX_RATES=4
ccflags-y += -DNX_CHAN_CTXT_CNT=3
ccflags-y += -DDEFAULT_COUNTRY_CODE=\"00\"
ccflags-y += -DAIC_TRACE_INCLUDE_PATH=$(src)

ccflags-y += -I$(srctree)/$(src)/../aic8800_bsp
ccflags-y += -Wno-implicit-fallthrough -Wno-unused-function -Wno-unused-variable

obj-$(CONFIG_AIC8800_WLAN_SUPPORT) += aic8800_fdrv.o
aic8800_fdrv-y := \
	rwnx_msg_tx.o \
	rwnx_msg_rx.o \
	rwnx_utils.o \
	rwnx_cmds.o \
	rwnx_irqs.o \
	rwnx_cfgfile.o \
	rwnx_strs.o \
	rwnx_rx.o \
	rwnx_tx.o \
	rwnx_txq.o \
	rwnx_main.o \
	rwnx_mod_params.o \
	rwnx_mesh.o \
	rwnx_platform.o \
	rwnx_dini.o \
	rwnx_v7.o \
	ipc_host.o \
	rwnx_tdls.o \
	aic_vendor.o \
	aicwf_compat_8800dc.o \
	aicwf_compat_8800d80.o \
	rwnx_wakelock.o \
	regdb.o \
	aic_priv_cmd.o \
	aicwf_rx_prealloc.o \
	rwnx_radar.o \
	sdio_host.o \
	aicwf_txrxif.o \
	aicwf_sdio.o \
	aicwf_tcp_ack.o
