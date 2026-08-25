/* rv1106 tsadc (TSADCV9 + VOGRF) register defs, ported from vendor */
#define TSADCV9_Q_MAX				0x210
#define TSADCV9_FLOW_CON			0x218
#define TSADCV9_AUTO_SRC			(0x10001 << 0)
#define TSADCV9_PD_MODE				(0x10001 << 4)
#define TSADCV9_Q_MAX_VAL			(0xffff0400 << 0)
#define RV1106_VOGRF_TSADC_CON			0x6000C
#define RV1106_VOGRF_TSADC_TSEN			(0x10001 << 8)
#define RV1106_VOGRF_TSADC_ANA			(0xff0007 << 0)


/**
 * struct tsadc_table - code to temperature conversion table
 * @code: the value of adc channel
 * @temp: the temperature
 * Note:
 * code to temperature mapping of the temperature sensor is a piece wise linear
 * curve.Any temperature, code faling between to 2 give temperatures can be
 * linearly interpolated.
 * Code to Temperature mapping should be updated based on manufacturer results.
 */
struct tsadc_table {
	u32 code;
	int temp;
};

static const struct tsadc_table rv1108_table[] = {
	{0, -40000},
	{374, -40000},
	{382, -35000},
	{389, -30000},
	{397, -25000},
	{405, -20000},
	{413, -15000},
	{421, -10000},
	{429, -5000},
	{436, 0},
	{444, 5000},
	{452, 10000},
	{460, 15000},
