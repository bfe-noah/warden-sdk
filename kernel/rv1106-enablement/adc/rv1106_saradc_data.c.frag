/* RV1106: a 2-channel v2 SARADC (ported from the vendor driver). */
static const struct iio_chan_spec rockchip_rv1106_saradc_iio_channels[] = {
	SARADC_CHANNEL(0, "adc0", 10),
	SARADC_CHANNEL(1, "adc1", 10),
};

static const struct rockchip_saradc_data rv1106_saradc_data = {
	.channels = rockchip_rv1106_saradc_iio_channels,
	.num_channels = ARRAY_SIZE(rockchip_rv1106_saradc_iio_channels),
	.clk_rate = 1000000,
	.start = rockchip_saradc_start_v2,
	.read = rockchip_saradc_read_v2,
};
