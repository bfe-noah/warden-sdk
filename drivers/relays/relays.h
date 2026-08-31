/**
 * The two onboard relays (86-Panel bottom board).
 *
 *     RELAY1  <-  GPIO1_A0   (sysfs gpio 32)
 *     RELAY2  <-  GPIO1_A1   (sysfs gpio 33)
 *
 * gpiochip1 base is 32; A0..A7 are offsets 0..7. Nothing in the DT claims these
 * (there is no relay driver), so we drive them through /sys/class/gpio directly.
 * An off-by-one here would toggle the neighbouring RS485 pair (GPIO1_B0/B1)
 * under a running Modbus master, so the numbers come from the schematic, not
 * inference. State is never assumed at startup: a relay may be holding a
 * contactor closed, and deciding it should be off because we just booted is not
 * this module's call.
 *
 * Hardened for warden-sdk: the sysfs plumbing sits behind a `relay_io` seam so
 * the decision logic runs and is measured to 100% MC/DC on the host, and the
 * gpio root is `$WARDEN_GPIO_ROOT`-overridable so the real backend can run
 * against a scratch tree in an integration test.
 */
#ifndef WARDEN_RELAYS_H
#define WARDEN_RELAYS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define WARDEN_RELAY_COUNT 2

/** Display name for @p idx, e.g. "Relay 1"; "-" if out of range. */
const char * warden_relay_name(uint32_t idx);

/** Current output state, false if it cannot be read. */
bool warden_relay_get(uint32_t idx);

/** Drive the output. Exports and sets the direction on first use. */
void warden_relay_set(uint32_t idx, bool on);

/** True if the GPIO is exported and usable: the page says so if it is not. */
bool warden_relay_available(uint32_t idx);

/* --- Hardware-abstraction seam ---------------------------------------------
 * The four filesystem primitives the relay logic needs. Production binds the
 * sysfs backend (the default); a unit test binds an in-memory fake that can
 * model export -> node-appears, which a passive scratch tree cannot. */
struct relay_io {
    bool (*exists)(const char *path);                      /* stat(path)==0 */
    void (*write)(const char *path, const char *value);    /* best-effort */
    bool (*read_line)(const char *path, char *buf, size_t n); /* a line was read */
    bool (*read_int)(const char *path, int *out);          /* one int parsed */
};

/** Swap the io backend (test hook). Pass NULL to restore the sysfs default. */
void warden_relay__set_io(const struct relay_io *io);

#endif /* WARDEN_RELAYS_H */
