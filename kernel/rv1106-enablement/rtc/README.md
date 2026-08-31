# RTC: rockchip,rv1106-rtc

`rtc-rockchip.c`: the vendor internal-RTC driver (open source, from the SDK),
ported to 6.18. Only 5.10->6.18 API delta: `rtc_register_device` ->
`devm_rtc_register_device` (paired with the existing `devm_rtc_allocate_device`).
Enable `CONFIG_RTC_DRV_ROCKCHIP=y` + `&rtc { status="okay"; }`. Verified on
warden-c8a3: `/dev/rtc0` registers and reads.
