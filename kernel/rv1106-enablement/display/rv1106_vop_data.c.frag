 * RV1106 VOP: the same RV-series "lite" VOP as rv1126 (VOP_VERSION 2.0xc vs
 * 2.0xb), so it reuses rv1126's register sub-structs. Only the version and the
 * smaller max raster differ. Ported for the WardenOS 86-Panel (720x720 RGB).
 */
static const struct vop_data rv1106_vop = {
	.version = VOP_VERSION(2, 0xc),
	.feature = VOP_FEATURE_INTERNAL_RGB,	/* RV1106 drives a parallel-RGB panel */
	.intr = &px30_intr,
	.common = &rv1126_common,
	.modeset = &rv1126_modeset,
	.output = &rv1126_output,
	.misc = &rv1126_misc,
	.win = rv1126_vop_win_data,
	.win_size = ARRAY_SIZE(rv1126_vop_win_data),
	.max_output = { 1280, 1280 },
	.lut_size = 1024,
};
