/* rv1106 tsadc (TSADCV9 + VOGRF) register defs, ported from vendor */
#define TSADCV9_Q_MAX				0x210
#define TSADCV9_FLOW_CON			0x218
#define TSADCV9_AUTO_SRC			(0x10001 << 0)
#define TSADCV9_PD_MODE				(0x10001 << 4)
#define TSADCV9_Q_MAX_VAL			(0xffff0400 << 0)
#define RV1106_VOGRF_TSADC_CON			0x6000C
#define RV1106_VOGRF_TSADC_TSEN			(0x10001 << 8)
#define RV1106_VOGRF_TSADC_ANA			(0xff0007 << 0)
