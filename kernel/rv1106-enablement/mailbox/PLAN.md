# HPMCU mailbox to 100% — plan, feasibility, hardware round-trip verify

**Goal:** a working, fully-open A7 ↔ HPMCU (SCR1) mailbox link on our self-built
Linux 6.18.46, **verified by a message round-trip over serial on hardware.**

**Bottom line up front:** this is genuinely achievable and far more tractable than
the NPU. Every piece is open (GPL kernel driver + our own bare-metal SCR1 firmware),
the controller driver **already binds on our 6.18 kernel with zero code change**,
and we already have a **hardware-validated way to load and run custom firmware on
the SCR1 core**. The only real work is: (1) enable the controller in DT + config,
(2) write a ~dozen-line SCR1 echo handler, (3) write a small Linux client, and
(4) run the round-trip on `warden-c8a3` (the recovery-capable rig) over serial.

The one substantive design decision — **hardware mailbox IP vs. our current
`/dev/mem` polled-SRAM scheme** — is settled below: build the real mailbox link
(it is the open, IRQ-driven, general-purpose answer), but keep the proven SRAM
watchdog exactly as-is (different threat model, different job).

---

## 1. Feasibility — honest assessment

**Fully feasible. Two halves, both open, both with working reference code to copy.**

- **Controller (Linux side): non-issue.** `drivers/mailbox/rockchip-mailbox.c` is
  upstream in mainline 6.18 and **already binds on our exact kernel** via the
  generic `rockchip,rk3368-mailbox` **fallback compatible** with **zero patching**
  — recorded in `CAPABILITIES-AUDIT.md:30`, confirmed by source read. RV1106's DT
  declares both instances with that fallback string. Gated today only by
  `status="disabled"` + `CONFIG_ROCKCHIP_MBOX` being absent from the defconfig.
- **HPMCU firmware (MCU side): we already do the hard part.** WardenOS has a
  **hardware-validated (2026-08-14, PR #25)** bare-metal SCR1 firmware built with
  the vendor's own `sysdrv/source/mcu/` toolchain (xPack `riscv-none-embed-gcc`
  10.2.0, `-march=rv32imc`), loaded via `/dev/mem`-mmap by `flared/src/hpmcu.rs`,
  running a real state machine and firing a real CRU reset. Adding a mailbox echo
  handler to that firmware is small, additive work — and the vendor ships a
  register-level template for the MCU side (`hal_mbox.c`) plus a working two-ended
  example (`battery-ipc/stream.c` + `rockchip_thunderboot_service.c`).
- **The only genuinely new/greenfield piece** is that no general Linux-side mailbox
  _client_ for RV1106 exists in-tree (the one real client,
  `rockchip_thunderboot_service.c`, is a hardwired one-shot `{0xf00d,0xdeadbeef}`
  "MCU done" signal, and isn't in our board's DT). But a mailbox client using the
  stock upstream `mailbox_client.h` API is a small, well-understood piece of code —
  not a driver port. `mailbox-test.c` (stock kernel debugfs exerciser, present in
  our tree, `compatible="mailbox-test"`) lets us prove the Linux→controller path
  **before writing any client at all.**

**Confidence in a verifiable open link: high.** The only area flagged risky is
_starting the SCR1 with our own entry point_ — and we have already solved and
hardware-proven exactly that (the `hpmcu.rs` load/release sequence), so it is a
known quantity here, not the open problem it would be for a team starting cold.

**Decision — hardware mailbox vs. `/dev/mem` SRAM:** build the **hardware mailbox**
as the open, general, IRQ-driven bidirectional channel (this is "the real mailbox
client" the task wants). **Keep the existing SRAM polled-word watchdog untouched** —
ADR-0002 deliberately chose it as a dead-man's-switch (no IRQ, no dependency on the
mailbox controller being up, survives A7 hangs by design). They coexist: the SRAM
word is the safety watchdog; the mailbox is the general IPC channel. Do **not** rip
out `hpmcu.rs`'s watchdog to route it through the mailbox — that would trade a
proven fail-safe for a more complex path with no safety gain.

**Do NOT (for a first cut):** port `rockchip_rpmsg.c` or NXP `rpmsg-lite`. Both are
real Rockchip patterns but genuinely un-ported to RV1106 (rpmsg-lite has platform
files only for RK3308/RK3568; `rockchip_rpmsg.c` matches only `rk3562/rk3568-rpmsg`;
neither is upstream in mainline; `CONFIG_RPMSG_ROCKCHIP` is `# not set` on every
RV1106 defconfig). They add virtio/vring machinery we don't need to prove a link.
If a richer multi-message channel is later wanted, layer a small hand-rolled ring
buffer in `hpmcu_sram` on top of the doorbell — that's the natural next step, not
a full rpmsg port.

---

## 2. Key technical facts

### The two mailbox instances (both `status="disabled"` in base `rv1106.dtsi`)

| Node | Base | IRQ | Clock | Consumer | Use |
|---|---|---|---|---|---|
| `mailbox@ff5c0000` | `0xff5c0000` (`reg` size `0x200`) | `GIC_SPI 1` | `PCLK_MAILBOX` | `thunder-boot-service` (ch1, `"amp-rx"`) — not on our board | **HPMCU-connected — use this one** |
| `pmu_mailbox@ff378000` | `0xff378000` (`0x200`) | `GIC_SPI 114` | `PCLK_PMU_MAILBOX` | none anywhere | PMU-domain, undocumented purpose — **avoid** |

**`@ff5c0000` is the HPMCU-connected instance, proven from both ends:** the Linux
`thunder_boot_service` node's `resets` are literally named `SRST_CORE_MCU*`; the
MCU-side vendor CMSIS header hard-codes `#define MBOX_BASE 0xFF5C0000U`
(`.../hal/lib/CMSIS/Device/RV1106/Include/rv1106.h:815`) — the same physical
address the A7 sees. Both clock IDs are real CRU gates (`PCLK_MAILBOX` at
`clk-rv1106.c:343`, `PCLK_PMU_MAILBOX` at `:737`), so the fallback-compatible match
is trustworthy: the register block is IP-identical to rk3368's.

### Register model (fixed offsets, hardcoded in the driver; identical both instances)

```
MAILBOX_A2B_INTEN   0x00        // AP -> MCU direction
MAILBOX_A2B_STATUS  0x04
MAILBOX_A2B_CMD(x)  0x08 + x*8  // x = channel 0..3
MAILBOX_A2B_DAT(x)  0x0c + x*8
MAILBOX_B2A_INTEN   0x28        // MCU -> AP direction
MAILBOX_B2A_STATUS  0x2C
MAILBOX_B2A_CMD(x)  0x30 + x*8
MAILBOX_B2A_DAT(x)  0x34 + x*8
```

**4 channels, 32-bit `{cmd,data}` per message per direction. Doorbell + 8-byte
payload — NOT a bulk channel.** Sender writes CMD then DAT (two `writel_relaxed`,
fire-and-forget, no ack/poll — `rockchip-mailbox.c:46-70`); the peer's STATUS bit
raises an IRQ; receiver reads CMD/DAT, dispatches to the registered client via
`mbox_chan_received_data`, clears the STATUS bit to ack. `#mbox-cells = <1>`
(channel index). Exported `rockchip_mbox_read_msg()` (`EXPORT_SYMBOL_GPL`,
`rockchip-mailbox.c:108-125`) pulls the last `{cmd,data}` pair out for a client.
Only OF match in the driver: `"rockchip,rk3368-mailbox"` → `.num_chans = 4`.

### HPMCU / SCR1 facts (from our own hardware-validated work)

- SCR1 = Syntacore RV32IMC, machine-mode only, 16KB unified cache, "HPMCU".
- **On our board the SCR1 is idle from reset** (non-TB `RV1106MINIALL.ini`, no
  `Hpmcu=` loader stage) — a clean slate, not even the stock camera-AE blob.
- **Usable MCU code SRAM is a hard 8KB**: `hpmcu_sram` at offset `0x3e000` inside
  `system_sram@ff6c0000` → absolute **`0xFF6FE000`, 8KB** (`reg=<0x3e000 0x2000>`,
  `rv1106.dtsi:1149-1151`), shared region with 248KB `rkisp_sram`. Our firmware +
  mailbox echo handler must fit this.
- **Proven load/release sequence** (`flared/src/hpmcu.rs:26-76`, hardware-validated):
  `CORECRU_CORESOFTRST_CON01` (`0xff3b8000+0xa04`) hold `0x1e001e` → write firmware
  to `0xff6fe000` → `SGRF_HPMCU_BOOT_ADDR` (`0xff076000+0x44`) = load addr →
  release CORECRU to `0x1e0000`. This is the WiFi-independent way we start the core
  with our own entry point — the piece source-level analysis flagged as unknown is
  already solved and proven here.
- **MCU-side reference to copy:** `hal_mbox.c`/`hal_mbox.h` (generic 4-channel
  register driver, `HAL_MBOX_Init`/send/ack) + working example
  `battery-ipc/stream.c:380-425` (sends `struct MBOX_CMD_DAT {cmd,data}` over
  `MBOX_CH_1`, client name `"mcu-status"` — exactly matches the Linux
  `struct rockchip_mbox_msg`).

### Current comms mechanism (what exists today, keep it)

WardenOS's watchdog uses a **software mailbox in shared SRAM**, not the mailbox IP:
last 256 bytes of `hpmcu_sram` at absolute base **`0xFF6FFF00`** — `+0x00 magic`
(Linux: `WARD`/`DISA`), `+0x04 counter` (heartbeat), `+0x08 mcu_state`
(`BOOT/ARMD/DISA/FIRE`), `+0x0c`/`+0x10` debug. 5s heartbeat, fires after 90s
no-advance. CRU reset via `GLB_SRST_FST` at **`0xff3b0c08`** magic `0xfdb9` (the
only working whole-SoC reset — `reboot -f` is a no-op: no PSCI/restart handler).
Reset constant is CI-guarded across 4 sites. **`dd of=/dev/mem` faults for the SRAM
region on this ARM kernel — only the `mmap()` path writes** (busybox `devmem` or
`libc::mmap`); `STRICT_DEVMEM` is off in our config.

### The `0x40000` boot-load hazard (load-bearing — for any _boot-time_ MCU path only)

Boot-loading firmware to the DDR carve-out `0x40000` (240KB) **bricked warden-c8a3
on 2026-08-23** because our non-TB kernel DT does **not** reserve `0x40000` — it
collides with kernel RAM → early-boot hang before eth0. **This mailbox plan avoids
the hazard entirely** by using the runtime SRAM-load path (`0xff6fe000`), not a
boot-time DDR load. If a boot-time path is ever pursued, a `reserved-memory` DT node
for `0x40000/0x3c000` must be added and verified via `/proc/iomem` **before**
flashing the idblock. (Standing memory: `boot-loaded-mcu-0x40000-hazard.md` —
worth promoting into `riscv-mcu.md` open-questions; not yet captured there.)

### URLs / upstream status

- `drivers/mailbox/rockchip-mailbox.c` — upstream mainline v6.18 (Bootlin Elixir
  confirms), binds via `rk3368-mailbox` fallback, zero patch.
- No `drivers/rpmsg/rockchip*` and no `drivers/remoteproc/rockchip*` in mainline
  (GitHub API enumeration of both dirs — none Rockchip). Vendor `rockchip_rpmsg.c`
  never merged; `lore.kernel.org` 403-walled, submission history unpinnable.
- Community RV1106 HPMCU prior art beyond LED-blink: **~zero** (GitHub search
  `rv1106 hpmcu` / `rv1106 scr1 coprocessor` → only SDK/rkbin mirrors). ADR-0002:
  "expect to be first." Our PR #25 firmware is the only known custom SCR1 code.

---

## 3. The simplest fully-open, hardware-verifiable path (milestones)

### M-MBOX-1 — Controller wiring proof (Tier 1: today, ZERO HPMCU risk, no firmware)

Proves DT status/clock/IRQ/probe/send all work on real hardware without touching
the SCR1 at all.

1. Kernel config: `CONFIG_ROCKCHIP_MBOX=y` + `CONFIG_MAILBOX_TEST=y` (mainline
   driver, **no patch** — `rockchip-mailbox.c` binds on the fallback compatible).
2. Board DTS: override `&mailbox { status = "okay"; };` (the `@ff5c0000` instance —
   main CRU clock, `GIC_SPI 1`, no PMU-domain complications). Add a
   `compatible = "mailbox-test"` node with `mboxes = <&mailbox N>` on a **free
   channel — avoid channel 1** (vendor reserves it for `"amp-rx"` semantics).
3. **Verify on serial console** (via the `warden-c8a3` `_b`-slot one-shot loop):
   confirm `rockchip-mailbox` probes clean in `dmesg`; write an 8-byte `{cmd,data}`
   pair to the debugfs `message` file (`mailbox-test.c`); then read back the
   physical `A2B_CMD(N)`/`A2B_DAT(N)` registers directly with busybox `devmem` at
   `0xff5c0008+8N` / `0xff5c000c+8N` and confirm the value landed. **Proves the AP→
   controller send path end-to-end, serial-only, no risk to the HPMCU.**

### M-MBOX-2 — Real round-trip: SCR1 echo + Linux client (Tier 2: the deliverable)

The actual "message round-trip over serial on hardware" the task asks for.

1. **SCR1 echo firmware** (additive to our existing bare-metal firmware, must fit
   the 8KB `hpmcu_sram` budget): on A2B IRQ (or a tight poll of `A2B_STATUS` — poll
   is simpler and safe for a bring-up echo, avoids MCU IRQ-controller setup), read
   `A2B_CMD(x)`/`A2B_DAT(x)`, write them back to `B2A_CMD(x)`/`B2A_DAT(x)`, set the
   `B2A_STATUS` bit, clear `A2B_STATUS`. Copy the register sequence from vendor
   `hal_mbox.c` / `battery-ipc/stream.c`. Load and start the core with the
   **proven `hpmcu.rs` load/release sequence** (SRAM path, `0xff6fe000` — **not**
   the `0x40000` boot path, hazard §2).
2. **Linux client**: a minimal in-tree `mbox_client` (stock `mailbox_client.h`:
   `mbox_request_channel_byname` / `mbox_send_message` + an rx callback reading via
   the exported `rockchip_mbox_read_msg()`), or — to avoid writing a kernel client
   for the first proof — reuse `mailbox-test`'s debugfs send and read the B2A
   registers from userspace with `devmem`. Prefer the `mailbox-test` route for
   M-MBOX-2's first light; promote to a real `mbox_client` once the round-trip is
   green.
3. **Verify on serial console** (`warden-c8a3`, `_b`-slot): send a known
   `{cmd,data}` (e.g. `{0x1234, 0xcafef00d}`) from Linux; confirm the SCR1 echoed
   it back — either via the client's rx callback logging the received pair, or by
   `devmem` reading `B2A_CMD(N)`/`B2A_DAT(N)` (`0xff5c0030+8N`/`0xff5c0034+8N`) and
   matching it byte-for-byte against what was sent. Capture the serial transcript
   as the evidence. **This is the 100%, verified, fully-open mailbox link.**

**Risk controls:** all boot testing via the `_b`-slot one-shot loop (never touches
the working `_a` slot, auto-reverts on hang). The SRAM-load path sidesteps the
`0x40000` brick hazard. The existing SRAM watchdog keeps running throughout,
independent of the mailbox, as it did during the 2026-08-14 validation.

### M-MBOX-3 — (Optional, later) richer channel

Only if a real multi-message need appears: layer a small ring buffer in
`hpmcu_sram`, mailbox used as the doorbell ("look at address X"). Still **do not**
adopt rpmsg/virtio unless the payload complexity genuinely demands it.

---

## 4. Risks / open questions

- **SCR1 IRQ setup** — for M-MBOX-2 a polled echo avoids configuring the MCU's
  interrupt controller; fine for bring-up. IRQ-driven B2A on the Linux side works
  regardless (that's the controller's job). Note as a simplification, not a gap.
- **Channel choice** — use a channel ≠ 1 (ch1 = vendor `"amp-rx"` reservation).
- **`pmu_mailbox@ff378000` purpose** — undocumented, no consumer anywhere; leave
  disabled, do not use, until a TRM section or Rockchip engineer clarifies.
- **RS-485 UART reachability from the HPMCU** — never checked (pinmux vs Linux
  ownership); irrelevant to the mailbox link but open for future MCU apps.
- **Whether Rockchip-official (non-RE) MCU/mailbox docs exist under NDA/partner
  access** — never asked Luckfox support directly; worth a query.
- **`mcutool.c` exact `/dev/mem` sequence** — inferred, not read line-by-line;
  moot, since our `hpmcu.rs` is an independent hardware-validated reimplementation.

---
_Cross-refs: `../CAPABILITIES-AUDIT.md:30`, `../REMAINING-PORTS.md §7`,
`../../luckfox-pico-86-panel/riscv-mcu.md`,
`.../raw/followup-riscv-mcu.md`,
`flare-edge/major-app-additions/docs/decisions/0002-hpmcu-watchdog.md`,
`flare-edge/major-app-additions/flared/src/hpmcu.rs`,
`flare-edge/major-app-additions/hpmcu/watchdog/main.c`,
`flare-edge/sdk/.../drivers/mailbox/rockchip-mailbox.c`,
`flare-edge/sdk/.../mcu/rt-thread/.../hal/lib/hal/src/hal_mbox.c`,
`battery-ipc/stream.c`. Standing memory: `boot-loaded-mcu-0x40000-hazard.md`._
