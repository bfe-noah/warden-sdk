# RV1106 Audio Codec: 5.10 (Rockchip BSP) -> 6.18.46 Port

Status: **zImage + rockchip/rv1106-warden.dtb build cleanly, 0 errors, 0 warnings**, with
`SND_SOC_RV1106=y` (acodec) and `SND_SOC_RK_DSM=y` (dsm) both built in, and the
`simple-audio-card` sound card wired in the board DT. Verified against `System.map` (host
`grep`, not cross-`nm` — see Verification below). **Not flashed or run on hardware** — this is
a build-only port; on-target audio verification is explicitly deferred to the parent
session / bench.

Trees involved:
- Target (edited): `<flare-edge>/research/linux-6.18.46/`
- Vendor source (read-only, copy-from): `<flare-edge>/sdk/sysdrv/source/kernel/` (Rockchip 5.10.160)

## Audio path

`i2s0_8ch` (DAI, already built/enabled pre-port) -> `acodec` (analog codec, this port) ->
speaker/headphone, tied together by a mainline `simple-audio-card`. The digital speaker
modulator (`dsm`) was also ported and builds cleanly, but is left **disabled** in the board DT,
matching the vendor 86-Panel board file (`rv1106-luckfox-pico-86panel-ipc.dtsi`), which also
ships `dsm` and `dsm_sound` disabled — the panel's actual audio path is acodec-only.

## Files copied (verbatim from vendor, then patched in place)

From `sdk/sysdrv/source/kernel/sound/soc/codecs/` to the 6.18 tree's `sound/soc/codecs/`:

| File | Lines | Notes |
|---|---|---|
| `rv1106_codec.c` | 2317 | Analog codec (acodec) driver — the priority target |
| `rv1106_codec.h` | — | Register/bitfield definitions, copied unmodified |
| `rk_dsm.c` | 653 | Digital speaker modulator (DSM) driver |
| `rk_dsm.h` | — | Register/bitfield definitions, copied unmodified |

## Kconfig / Makefile wiring

`sound/soc/codecs/Kconfig`:
- Added `config SND_SOC_RK_DSM` (tristate, `depends on ARCH_ROCKCHIP || COMPILE_TEST`) after
  `SND_SOC_RK817`, before `SND_SOC_RL6231`.
- Added `config SND_SOC_RV1106` (tristate, `depends on ARCH_ROCKCHIP || COMPILE_TEST`,
  `select REGMAP_MMIO`) after `SND_SOC_RTQ9128`, before `SND_SOC_SDW_MOCKUP` (alphabetical
  slot for "RV").
- Neither entry restricts to `ARM64` (unlike `SND_SOC_RK3308`'s existing
  `depends on ARM64 || COMPILE_TEST` in this tree) — RV1106 is Cortex-A7 / ARM32-only, so an
  ARM64 dependency would make the symbol unselectable on this board's `ARCH=arm` build.

`sound/soc/codecs/Makefile` (6.18 uses `snd-soc-<name>-y := <objs>.o` instead of the vendor's
`snd-soc-<name>-objs := <objs>.o` — same semantics, current tree's naming convention followed):
- `snd-soc-rk-dsm-y := rk_dsm.o` + `obj-$(CONFIG_SND_SOC_RK_DSM) += snd-soc-rk-dsm.o`
- `snd-soc-rv1106-y := rv1106_codec.o` + `obj-$(CONFIG_SND_SOC_RV1106) += snd-soc-rv1106.o`

## Config symbols set

Via `./scripts/config --enable <SYMBOL>` then `make ARCH=arm olddefconfig`, confirmed `=y` in
`.config` afterwards (no silent Kconfig-dependency drops):

```
CONFIG_SOUND=y
CONFIG_SND=y
CONFIG_SND_SOC=y
CONFIG_SND_SOC_RV1106=y
CONFIG_SND_SOC_RK_DSM=y
CONFIG_SND_SIMPLE_CARD=y
CONFIG_SND_SIMPLE_CARD_UTILS=y
CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM=y   (already y — DMA glue for the DAI)
```

Note: `SOUND`/`SND`/`SND_SOC` were `=m` in the pre-port `.config` (the whole ALSA/ASoC
subsystem was module-only). They had to be flipped to `=y` first — a tristate symbol can't be
`y` while its dependency chain is capped at `m` — otherwise `SND_SOC_RV1106=y` would silently
clamp to `m` under `olddefconfig`, and `System.map` (built-in symbols only) would show nothing
even with `grep -c rv1106_codec` returning 0, which is exactly the silent-drop failure mode the
task asked to guard against. `REGMAP_MMIO` and `MFD_SYSCON` were already `=y` (pulled in by
other on-SoC drivers), so no additional dependency chasing was needed for those.

## API-delta fixes (5.10 BSP -> 6.18 mainline ASoC)

The vendor 5.10.160 BSP already tracks a fairly modern ASoC callback shape (component-based
`.probe`/`.remove`, `snd_soc_dai_ops` with `.mute_stream(dai, mute, stream)`, etc.), so the
actual delta surface was small. Six fixes total, all found by iterating single-object builds
(`make ... sound/soc/codecs/rv1106_codec.o`) before doing the full `zImage` build:

| # | File | Change |
|---|---|---|
| 1 | `rv1106_codec.c` | Dropped `#include <linux/rockchip/grf.h>` — vendor-BSP-only header, not present in mainline. Nothing in the file actually needs symbols from it: `PERI_GRF_PERI_CON1` is defined locally in the driver and the GRF is accessed generically via `syscon_regmap_lookup_by_phandle()` + `regmap_write()`. Documented in a comment at the top of the include block. |
| 2 | `rv1106_codec.c` | Replaced `#include <linux/of_gpio.h>` with `#include <linux/gpio/consumer.h>`. The driver calls `devm_gpiod_get_optional()` / `gpiod_direction_output()` (the `gpiod_*` consumer API), which live in `gpio/consumer.h`, not `of_gpio.h` (legacy integer-GPIO header) — this only happened to resolve on 5.10 via a transitive include chain that no longer holds in 6.18. |
| 3 | `rv1106_codec.c` | Added `#include <linux/of_device.h>` — `of_match_device()` is declared there, not pulled in transitively by `of_platform.h` any more; without it the call was an implicit-function-declaration error (`-Werror`) plus a pointer-from-int warning on the assignment. |
| 4 | `rv1106_codec.c` | `snd_soc_get_volsw_range()` / `snd_soc_put_volsw_range()` (used in `rv1106_codec_mic_gain_get/put` and `rv1106_codec_hpmix_gain_get/put`) no longer exist as separate mainline entry points — they were folded away when `SOC_SINGLE_RANGE`/`SOC_SINGLE_RANGE_TLV` were made simple aliases over `SOC_SINGLE_VALUE`. The unified `snd_soc_get_volsw()`/`snd_soc_put_volsw()` already honour the `xmin` field of `struct soc_mixer_control`, so they're a drop-in replacement; confirmed by checking how the affected controls are declared (`SOC_SINGLE_EXT_TLV`, min=0) — behavior is identical. 4 call sites fixed. |
| 5 | `rv1106_codec.c` | `SND_SOC_DAIFMT_CBS_CFS` -> `SND_SOC_DAIFMT_CBC_CFC`, `SND_SOC_DAIFMT_CBM_CFM` -> `SND_SOC_DAIFMT_CBP_CFP` in `rv1106_set_dai_fmt()`'s clock-role switch. Mainline renamed the old master/slave (`CBM`/`CBS`) constants to the consumer/provider naming (`CBC`/`CBP`) some releases back; `SND_SOC_DAIFMT_MASTER_MASK` itself still exists as a compat alias for `CLOCK_PROVIDER_MASK`, so only the two case labels needed renaming. Verified the semantic mapping against the branch bodies (codec-slave -> `IO_MODE_SLAVE`/`MODE_SLAVE` register bits = consumer = `CBC_CFC`; codec-master -> `..._MASTER` bits = provider = `CBP_CFP`). |
| 6 | `rv1106_codec.c` + `rk_dsm.c` | `struct platform_driver.remove` is `void (*remove)(struct platform_device *)` in 6.18 (was `int (*remove)(struct platform_device *)`, transitional `.remove_new` before that). Changed `rv1106_platform_remove()` and `rk_dsm_platform_remove()` from `static int ... { ...; return 0; }` to `static void ...` (dropped the trailing `return 0;`). This was an `-Werror=incompatible-pointer-types` build error, not a warning. |

That's the complete fix list — nothing else needed touching. No vendor-only headers other than
`rockchip/grf.h` were pulled in; `linux/mfd/syscon.h`, `linux/reset.h`, `linux/clk.h`,
`linux/regulator/consumer.h`, `sound/pcm_params.h`, `sound/soc.h`, `sound/tlv.h`,
`sound/dmaengine_pcm.h` all exist unchanged in mainline and needed no stubbing.

## rk_dsm (digital speaker modulator) decision

**Kept in, not dropped.** It only needed fix #6 above (the same `platform_driver.remove` void
signature change as the acodec) — a one-line-class fix, well under the "drop if it fights the
API a lot" threshold in the task brief. `SND_SOC_RK_DSM=y` is set and `rk_dsm.c` links cleanly
into `zImage` (17 `rk_dsm`-prefixed symbols in `System.map`, including
`rk_dsm_platform_remove`, `rk_dsm_hw_params`, `rk_dsm_set_dai_fmt`).

However, the DSM node (`dsm: codec-digital@ff340000`) and its companion `dsm_sound` machine
card are **not wired up** in `rv1106-warden.dts` — `&dsm { status = "disabled"; };` is set
explicitly, matching the vendor `rv1106-luckfox-pico-86panel-ipc.dtsi`, which itself ships `dsm`
disabled and `dsm_sound` as `status = "disabled"`. The 86-Panel's speaker/headphone path is the
acodec, not the DSM Class-D path; DSM is present on other RV1106 boards (bare speaker modules)
but not this one. The driver is built and available (a future DT change is all that's needed to
turn it on) but is not part of this board's active audio graph.

## Device tree changes (`arch/arm/boot/dts/rockchip/rv1106-warden.dts`)

`i2s0_8ch` was already `status = "okay"` pre-port (from an earlier "sweep" commit) and already
carries `#sound-dai-cells = <0>` in the base `rv1106.dtsi` node — no change needed there.

The base `rv1106.dtsi` `acodec: acodec@ff480000` node (mirrored from the vendor's own
`arch/arm/boot/dts/rv1106.dtsi`, which has the same gap) does **not** carry
`#sound-dai-cells`, unlike `i2s0_8ch` and `dsm`, which both already have it in the base dtsi.
The vendor board overlay (`rv1106-luckfox-pico-86panel-ipc.dtsi`) adds it via override, so this
port does the same, appended at the end of `rv1106-warden.dts` after the existing GMAC block:

```dts
&acodec {
	#sound-dai-cells = <0>;
	status = "okay";
};

&dsm {
	status = "disabled";
};

/ {
	acodec_sound: acodec-sound {
		compatible = "simple-audio-card";
		simple-audio-card,name = "rv1106-acodec";
		simple-audio-card,format = "i2s";
		simple-audio-card,mclk-fs = <256>;
		simple-audio-card,cpu {
			sound-dai = <&i2s0_8ch>;
		};
		simple-audio-card,codec {
			sound-dai = <&acodec>;
		};
	};
};
```

Verified against the decompiled built `.dtb` (`dtc -I dtb -O dts`):
- `acodec@ff480000`: `status = "okay"`, `#sound-dai-cells = <0x00>`, `phandle = <0x5a>`
- `i2s@ffae0000` (i2s0_8ch): `status = "okay"`, `#sound-dai-cells = <0x00>`, `phandle = <0x59>`
- `codec-digital@ff340000` (dsm): `status = "disabled"`
- `acodec-sound` node present with `sound-dai = <0x59>` (cpu) / `sound-dai = <0x5a>` (codec) —
  phandles correctly resolve to the i2s0_8ch/acodec nodes above.

## Build status

Exact commands run (per the task's build recipe):

```sh
export PATH="<flare-edge>/sdk/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf/bin:/usr/bin:/bin"
export ARCH=arm CROSS_COMPILE=arm-rockchip830-linux-uclibcgnueabihf-
cd <flare-edge>/research/linux-6.18.46
make ARCH=arm CROSS_COMPILE=$CROSS_COMPILE zImage rockchip/rv1106-warden.dtb -j"$(nproc)"
```

Result: **exit 0, 0 errors, 0 warnings** (a `grep -iE "error|warn"` over the full build log for
both the touched-file single-object builds and the final combined `zImage`+dtb build returned
nothing). `arch/arm/boot/zImage` and `arch/arm/boot/dts/rockchip/rv1106-warden.dtb` both
produced. XZ compression and the existing console/earlycon config were left untouched.

## Verification (build-only — see hardware watch-list below for what's deferred)

Per the task instructions, `System.map` (produced by the **host** toolchain's `nm`, ground
truth for built-in linkage — not the cross-`nm`, which mis-lists symbols) was checked directly
with `grep`:

```
$ grep -c rv1106_codec System.map
54
$ grep -c rk_dsm System.map
17
$ grep -c soc_codec_dev_rv1106 System.map
1
```

54 `rv1106_codec`-prefixed symbols and 17 `rk_dsm`-prefixed symbols are linked into the kernel
image — both drivers are genuinely built in (`=y`), not silently dropped to module or excluded.
`soc_codec_dev_rv1106` (the `snd_soc_component_driver` struct) is present, confirming the
component registration path is compiled in.

## Hardware-verify watch-list (deferred to the parent session / bench — NOT done here)

This session did not flash or touch any hardware, per the task boundary. When the parent
session verifies on a real 86-Panel:

- `aplay -l` should list a card named **`rv1106-acodec`** (from
  `simple-audio-card,name = "rv1106-acodec"` in the DT) — this is the ALSA card name to look
  for, not a device path.
- Expect one playback + one capture PCM stream under that card (`rv1106-hifi` DAI: playback 1-2ch,
  capture 1-4ch, 8kHz-192kHz, S16_LE/S20_3LE/S24_LE/S32_LE).
- `amixer -c <card> controls` should surface the vendor mixer controls carried over unchanged
  from the BSP driver (ADC MIC Left/Right Gain, ADC ALC Left/Right Volume, HPF cutoff, DAC/lineout
  gain, mic bias, etc.) — these are pure regmap register pokes, unaffected by this port's API-shim
  fixes, so if the card enumerates at all they should already work correctly.
- Watch dmesg for probe-order issues: `acodec` depends on `PCLK_ACODEC`, `MCLK_ACODEC_TX`,
  `MCLK_I2S0_8CH_TX` clocks and `SRST_P_ACODEC` reset all resolving before probe; a probe
  deferral (`-EPROBE_DEFER`) once at boot before the CRU driver is up is normal, a permanent
  failure is not.
- If `aplay -l` shows no card at all: check `dmesg` for `simple-audio-card` DAI-link binding
  failures first (a `#sound-dai-cells` mismatch would show as
  "ASoC: ... error getting cpu/codec dai info" at parse time) before suspecting the codec driver
  itself, since the DT wiring is the newest/least-proven part of this port.
- The actual audible test (does sound come out of the speaker/headphone jack) is explicitly
  **deferred to the bench** — this session only proves the driver builds, links, and the DT
  graph resolves; it does not and cannot prove the analog output path works on real silicon.

---
## Parent-session verify + fix (2026-08-25)
**Gap found:** the subagent set `SND_SOC_RV1106=y` + `SND_SIMPLE_CARD=y` but left
the **cpu DAI** `CONFIG_SND_SOC_ROCKCHIP_I2S_TDM=m` (module). On the bare-kernel
_b boot no modules load, so i2s0_8ch never registered a DAI → the simple-audio-card
stuck in `deferred probe pending: asoc-simple-card: parse error`, no card.
**Fix:** `CONFIG_SND_SOC_ROCKCHIP=y` + `CONFIG_SND_SOC_ROCKCHIP_I2S_TDM=y` (the
i2s0_8ch node already carries the `rockchip,rv1126-i2s-tdm` fallback compatible +
`#sound-dai-cells=<0>`, so the mainline driver binds it).

**VERIFIED on c8a3:** `/proc/asound/cards` → `0 [rv1106acodec]: simple-card -
rv1106-acodec`; `aplay -l` → `card 0: rv1106acodec, device 0:
ffae0000.i2s-rv1106-hifi`; `/dev/snd/` has `controlC0 pcmC0D0p pcmC0D0c` (playback
+ capture). Audible speaker test deferred to the bench (with the display).
