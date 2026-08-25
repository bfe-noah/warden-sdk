/* --- rv1106 tsadc port (from vendor) --- */
static const struct tsadc_table rv1106_code_table[] = {
	{0, MIN_TEMP},
	{363, MIN_TEMP},
	{396, -40000},
	{504, 25000},
	{605, 85000},
	{673, 125000},
	{758, MAX_TEMP},
	{TSADCV2_DATA_MASK, MAX_TEMP},
};

static void rk_tsadcv9_initialize(struct regmap *grf, void __iomem *regs,
				  enum tshut_polarity tshut_polarity)
{
	regmap_write(grf, RV1106_VOGRF_TSADC_CON, RV1106_VOGRF_TSADC_TSEN);
	udelay(10);
	regmap_write(grf, RV1106_VOGRF_TSADC_CON, RV1106_VOGRF_TSADC_ANA);
	udelay(100);

	writel_relaxed(TSADCV2_AUTO_PERIOD_TIME, regs + TSADCV3_AUTO_PERIOD);
	writel_relaxed(TSADCV2_AUTO_PERIOD_TIME,
		       regs + TSADCV3_AUTO_PERIOD_HT);
	writel_relaxed(TSADCV2_HIGHT_INT_DEBOUNCE_COUNT,
		       regs + TSADCV3_HIGHT_INT_DEBOUNCE);
	writel_relaxed(TSADCV2_HIGHT_TSHUT_DEBOUNCE_COUNT,
		       regs + TSADCV3_HIGHT_TSHUT_DEBOUNCE);
	writel_relaxed(TSADCV9_AUTO_SRC, regs + TSADCV2_INT_PD);
	writel_relaxed(TSADCV9_PD_MODE, regs + TSADCV9_FLOW_CON);
	writel_relaxed(TSADCV9_Q_MAX_VAL, regs + TSADCV9_Q_MAX);
	if (tshut_polarity == TSHUT_HIGH_ACTIVE)
		writel_relaxed(TSADCV2_AUTO_TSHUT_POLARITY_HIGH |
			       TSADCV2_AUTO_TSHUT_POLARITY_MASK,
			       regs + TSADCV2_AUTO_CON);
	else
		writel_relaxed(TSADCV2_AUTO_TSHUT_POLARITY_MASK,
			       regs + TSADCV2_AUTO_CON);
	writel_relaxed(TSADCV3_AUTO_Q_SEL_EN | (TSADCV3_AUTO_Q_SEL_EN << 16),
		       regs + TSADCV2_AUTO_CON);
}

static const struct rockchip_tsadc_chip rv1106_tsadc_data = {
	/* top, big_core0, big_core1, little_core, center, gpu, npu */
	.chn_id[SENSOR_CPU] = 0, /* cpu sensor is channel 0 */
	.chn_num = 1, /* seven channels for tsadc */
	.tshut_mode = TSHUT_MODE_CRU, /* default TSHUT via CRU */
	.tshut_polarity = TSHUT_LOW_ACTIVE, /* default TSHUT LOW ACTIVE */
	.tshut_temp = 95000,
	.initialize = rk_tsadcv9_initialize,
	.irq_ack = rk_tsadcv4_irq_ack,
	.control = rk_tsadcv4_control,
	.get_temp = rk_tsadcv4_get_temp,
	.set_alarm_temp = rk_tsadcv3_alarm_temp,
	.set_tshut_temp = rk_tsadcv3_tshut_temp,
	.set_tshut_mode = rk_tsadcv4_tshut_mode,
	.table = {
		.id = rv1106_code_table,
		.length = ARRAY_SIZE(rv1106_code_table),
		.data_mask = TSADCV2_DATA_MASK,
		.mode = ADC_INCREMENT,
	},
};
