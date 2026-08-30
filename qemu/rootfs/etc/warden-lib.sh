# shellcheck shell=sh
# Shared helpers for the VM's stage-1 (/init + /etc/rc, initramfs) and stage-2
# (/sbin/init, disk rootfs) boot scripts. Present in both filesystems because
# both are staged from the same qemu/rootfs/ skeleton. ONE copy of each rule —
# the slot-validation drift between two hand-copied parsers was a real
# review finding.

# Populate /dev/block/by-name/<PARTNAME> symlinks from sysfs uevents — the
# contract flare-edge's slotctl.rs relies on. blkdevparts= gives every vda
# partition a PARTNAME.
warden_populate_by_name() {
    mkdir -p /dev/block/by-name
    for uev in /sys/class/block/vda*/uevent; do
        [ -f "$uev" ] || continue
        partname=""
        devname=""
        while IFS='=' read -r k v; do
            case "$k" in
                PARTNAME) partname="$v" ;;
                DEVNAME)  devname="$v" ;;
            esac
        done < "$uev"
        [ -n "$partname" ] && [ -n "$devname" ] \
            && ln -sf "/dev/$devname" "/dev/block/by-name/$partname"
    done
}

# Parse warden.slot= from the cmdline (whole-token, never substring) and
# VALIDATE it — echoes "_a" or "_b", falling back to _a with a warning.
warden_slot() {
    slot="_a"
    for tok in $(cat /proc/cmdline); do
        case "$tok" in
            warden.slot=*) slot="${tok#warden.slot=}" ;;
        esac
    done
    case "$slot" in
        _a|_b) ;;
        *) echo "warden-lib: bad warden.slot='$slot', falling back to _a" >&2; slot="_a" ;;
    esac
    echo "$slot"
}
