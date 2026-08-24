# config-lint — static target-config gates

Catches flash-time config faults the behavioural sim cannot: mistakes in the
*memory map*, not the logic. The first check is the one that would have caught the
**c8a3 brick** — a boot-loaded coprocessor firmware dropped at `0x40000`, which is
a `reserved-memory` carve-out on Thunder-Boot boards but plain kernel RAM on ours,
so the MCU and the kernel fought over the same DRAM and the board hung before eth0.

## The check

Every address the idblock loader drops MCU firmware to must sit inside a
`reserved-memory` node in the target devicetree.

- **MCU loads** come from the rkbin loader `.ini`: each `LOADERn=Hpmcu` (any
  hpmcu/mcu/amp entry) in `[LOADER_OPTION]`, with its `LOAD_ADDR` from
  `[LOADERn_PARAM]`.
- **Reserved ranges** come from the devicetree: every `reg = <addr size>` inside a
  `reserved-memory { … }` node.

A load outside all reservations is a failure (non-zero exit).

## Use

    cargo run -p warden-config-lint -- --ini <loader.ini> --dt <devicetree.dts>

In CI, feed the *flattened* devicetree so includes and overlays are resolved:

    dtc -I dtb -O dts build/.../rv1106g-warden.dtb > /tmp/warden.dts
    config-lint --ini .../RKBOOT/RV1106MINIALL*.ini --dt /tmp/warden.dts

Exit `0` = every MCU load is reserved (or there are none); `1` = a collision was
found; `2` = usage/IO error.

## Test

    cargo test -p warden-config-lint

The suite encodes the brick as a regression: the real Thunder-Boot `.ini`
(Hpmcu @ `0x40000`) *fails* against a DT with no `rtos@40000` node and *passes*
once the reservation is added — and our board's non-TB loader (no boot-loaded MCU)
always passes. See `../../docs/architecture.md` §5 and, for the hardware hazard,
the `boot-loaded-mcu-0x40000-hazard` note.
