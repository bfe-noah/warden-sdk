 * RV1106 USB2 PHY: single OTG port at 0xff3e0000 (ported from the vendor driver;
 * fields map 1:1 to 6.18 except utmi_iddig -> utmi_id, and the 5.10-only
 * iddig_output/iddig_en/bvalid_grf_sel are dropped). Signal-quality phy_tuning is
 * left off for now (guarded, so NULL is safe) — the phy is functional without it.
 */
static const struct rockchip_usb2phy_cfg rv1106_phy_cfgs[] = {
	{
		.reg = 0xff3e0000,
		.num_ports	= 1,
		.clkout_ctl	= { 0x0058, 4, 4, 1, 0 },
		.port_cfgs	= {
			[USB2PHY_PORT_OTG] = {
				.phy_sus	= { 0x0050, 8, 0, 0, 0x1d1 },
				.bvalid_det_en	= { 0x0100, 2, 2, 0, 1 },
				.bvalid_det_st	= { 0x0104, 2, 2, 0, 1 },
				.bvalid_det_clr = { 0x0108, 2, 2, 0, 1 },
				.idfall_det_en	= { 0x0100, 5, 5, 0, 1 },
				.idfall_det_st	= { 0x0104, 5, 5, 0, 1 },
				.idfall_det_clr = { 0x0108, 5, 5, 0, 1 },
				.idrise_det_en	= { 0x0100, 4, 4, 0, 1 },
				.idrise_det_st	= { 0x0104, 4, 4, 0, 1 },
				.idrise_det_clr = { 0x0108, 4, 4, 0, 1 },
				.ls_det_en	= { 0x0100, 0, 0, 0, 1 },
				.ls_det_st	= { 0x0104, 0, 0, 0, 1 },
				.ls_det_clr	= { 0x0108, 0, 0, 0, 1 },
				.utmi_avalid	= { 0x0060, 10, 10, 0, 1 },
				.utmi_bvalid	= { 0x0060, 9, 9, 0, 1 },
				.utmi_id	= { 0x0060, 6, 6, 0, 1 },
				.utmi_ls	= { 0x0060, 5, 4, 0, 1 },
			},
		},
		.chg_det = {
			.opmode		= { 0x0050, 8, 0, 0, 0x1d7 },
			.cp_det		= { 0x0060, 13, 13, 0, 1 },
			.dcp_det	= { 0x0060, 12, 12, 0, 1 },
			.dp_det		= { 0x0060, 14, 14, 0, 1 },
			.idm_sink_en	= { 0x0058, 8, 8, 0, 1 },
			.idp_sink_en	= { 0x0058, 7, 7, 0, 1 },
			.idp_src_en	= { 0x0058, 9, 9, 0, 1 },
			.rdm_pdwn_en	= { 0x0058, 10, 10, 0, 1 },
			.vdm_src_en	= { 0x0058, 12, 12, 0, 1 },
			.vdp_src_en	= { 0x0058, 11, 11, 0, 1 },
		},
	},
	{ /* sentinel */ }
};

static const struct of_device_id rockchip_usb2phy_dt_match[] = {
	{ .compatible = "rockchip,rv1106-usb2phy", .data = &rv1106_phy_cfgs },
	{ .compatible = "rockchip,px30-usb2phy", .data = &rk3328_phy_cfgs },
	{ .compatible = "rockchip,rk3036-usb2phy", .data = &rk3036_phy_cfgs },
	{ .compatible = "rockchip,rk3128-usb2phy", .data = &rk3128_phy_cfgs },
	{ .compatible = "rockchip,rk3228-usb2phy", .data = &rk3228_phy_cfgs },
	{ .compatible = "rockchip,rk3308-usb2phy", .data = &rk3308_phy_cfgs },
	{ .compatible = "rockchip,rk3328-usb2phy", .data = &rk3328_phy_cfgs },
	{ .compatible = "rockchip,rk3366-usb2phy", .data = &rk3366_phy_cfgs },
	{ .compatible = "rockchip,rk3399-usb2phy", .data = &rk3399_phy_cfgs },
	{ .compatible = "rockchip,rk3562-usb2phy", .data = &rk3562_phy_cfgs },
	{ .compatible = "rockchip,rk3568-usb2phy", .data = &rk3568_phy_cfgs },
	{ .compatible = "rockchip,rk3576-usb2phy", .data = &rk3576_phy_cfgs },
	{ .compatible = "rockchip,rk3588-usb2phy", .data = &rk3588_phy_cfgs },
	{ .compatible = "rockchip,rv1108-usb2phy", .data = &rv1108_phy_cfgs },
	{}
};
MODULE_DEVICE_TABLE(of, rockchip_usb2phy_dt_match);

static struct platform_driver rockchip_usb2phy_driver = {
	.probe		= rockchip_usb2phy_probe,
	.driver		= {
		.name	= "rockchip-usb2phy",
		.of_match_table = rockchip_usb2phy_dt_match,
	},
};
module_platform_driver(rockchip_usb2phy_driver);

MODULE_AUTHOR("Frank Wang <frank.wang@rock-chips.com>");
MODULE_DESCRIPTION("Rockchip USB2.0 PHY driver");
MODULE_LICENSE("GPL v2");
