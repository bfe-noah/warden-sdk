# qemu/payload/ — guest binaries (never committed)

Drop **static musl armv7** binaries here; `qemu/mkimage.sh` copies everything
in this directory (except this README) into `/usr/bin/` of both rootfs slots.
Static musl is the same target the device uses for its Rust daemons, so the
exact production binaries run unmodified in the VM.

Typical payload, built in a flare-edge checkout:

```sh
# flared (static musl armv7)
tools/build-flared.sh --local
# warden-modbus and friends: see tools/build-firmware.sh for the recipes
```

Then:

```sh
cp <flare-edge>/target/armv7-unknown-linux-musleabihf/release/warden-flared qemu/payload/
```

Stage-2 init starts `warden-flared`, `warden-modbus`, and `warden-ui` (the
UI additionally needs `--display on|headless` + the virt.fragment kernel for
/dev/fb0) automatically when present (logs land in `/tmp/<name>.log` inside
the guest). An empty payload is valid — the image boots busybox-only.
