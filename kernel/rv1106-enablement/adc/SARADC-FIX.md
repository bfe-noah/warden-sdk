# SARADC — VERIFIED on warden-c8a3 (2026-08-25); the -22 was vref, not clk

The rockchip_saradc probe failed `-22` NOT at clk_set_rate (no "failed to set
adc clk rate" ever printed) but at `regulator_get_voltage(info->vref)` — with no
`vref-supply` in DT the driver got a **dummy** regulator, and
`regulator_get_voltage(dummy)` returns -EINVAL, which probe returns directly (so
only the generic "probe failed with error -22" showed). The earlier
clk-rv1106-divider theory was wrong (the divider is HIWORD settable, xin24m is
registered; clk_set_rate would clamp, not fail).

**Fix (DT only):** add the 1.8 V reference the vendor 86-panel uses —
```dts
/ { vcc_1v8: vcc-1v8 { compatible = "regulator-fixed"; regulator-name = "vcc_1v8";
      regulator-always-on; regulator-boot-on;
      regulator-min-microvolt = <1800000>; regulator-max-microvolt = <1800000>; }; };
&saradc { vref-supply = <&vcc_1v8>; status = "okay"; };
```
**Evidence:** `iio:device0` (`ff3c0000.saradc`), `in_voltage0_raw=1023`,
`in_voltage1_raw=246` — both channels read real analog values (the adc-keys path).
