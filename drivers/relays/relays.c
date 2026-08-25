#include "relays.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

/* GPIO1_A0 and GPIO1_A1; gpiochip1 base is 32. See relays.h. */
static const int  s_gpio[WARDEN_RELAY_COUNT] = { 32, 33 };
static const char * const s_name[WARDEN_RELAY_COUNT] = { "Relay 1", "Relay 2" };

/* ---- default sysfs backend (Tier-2 plumbing; covered by integration tests) ---- */

static const char * gpio_root(void)
{
    const char * r = getenv("WARDEN_GPIO_ROOT");
    return (r && *r) ? r : "/sys/class/gpio";
}

static bool sysfs_exists(const char * path)
{
    struct stat st;
    return stat(path, &st) == 0;
}

static void sysfs_write(const char * path, const char * value)
{
    FILE * f = fopen(path, "w");
    if(!f) return;
    fputs(value, f);
    fclose(f);
}

static bool sysfs_read_line(const char * path, char * buf, size_t n)
{
    FILE * f = fopen(path, "r");
    if(!f) return false;
    bool ok = fgets(buf, (int)n, f) != NULL;
    fclose(f);
    return ok;
}

static bool sysfs_read_int(const char * path, int * out)
{
    FILE * f = fopen(path, "r");
    if(!f) return false;
    bool ok = fscanf(f, "%d", out) == 1;
    fclose(f);
    return ok;
}

static const struct relay_io s_sysfs_io = {
    sysfs_exists, sysfs_write, sysfs_read_line, sysfs_read_int
};

static const struct relay_io * s_io = &s_sysfs_io;

void warden_relay__set_io(const struct relay_io * io) { s_io = io ? io : &s_sysfs_io; }

/* ---- decision logic (Tier-1; measured to 100% MC/DC via the fake io) ---- */

const char * warden_relay_name(uint32_t idx)
{
    return idx < WARDEN_RELAY_COUNT ? s_name[idx] : "-";
}

static bool exported(int gpio)
{
    char p[80];
    snprintf(p, sizeof(p), "%s/gpio%d", gpio_root(), gpio);
    return s_io->exists(p);
}

/**
 * Export and set the direction, without disturbing the level. "out" on an
 * unexported pin latches the kernel's default low, which would drop a closed
 * relay the first time this page is opened; reading the current level first and
 * writing it straight back makes the export transparent.
 */
static bool ensure_ready(int gpio)
{
    if(!exported(gpio)) {
        char ex[80], n[16];
        snprintf(ex, sizeof(ex), "%s/export", gpio_root());
        snprintf(n, sizeof(n), "%d", gpio);
        s_io->write(ex, n);
        if(!exported(gpio)) return false;
    }

    char p[96];
    snprintf(p, sizeof(p), "%s/gpio%d/direction", gpio_root(), gpio);
    char dir[16] = "";
    if(!s_io->read_line(p, dir, sizeof(dir))) return false;

    if(strncmp(dir, "out", 3) != 0) {
        /* Preserve the level across the switch to output. */
        char vp[96];
        int cur;
        snprintf(vp, sizeof(vp), "%s/gpio%d/value", gpio_root(), gpio);
        if(!s_io->read_int(vp, &cur)) cur = 0;
        s_io->write(p, cur ? "high" : "low");
    }
    return true;
}

bool warden_relay_available(uint32_t idx)
{
    if(idx >= WARDEN_RELAY_COUNT) return false;
    return ensure_ready(s_gpio[idx]);
}

bool warden_relay_get(uint32_t idx)
{
    if(idx >= WARDEN_RELAY_COUNT) return false;
    if(!exported(s_gpio[idx])) return false;

    char p[96];
    int v;
    snprintf(p, sizeof(p), "%s/gpio%d/value", gpio_root(), s_gpio[idx]);
    if(!s_io->read_int(p, &v)) v = 0;
    return v != 0;
}

void warden_relay_set(uint32_t idx, bool on)
{
    if(idx >= WARDEN_RELAY_COUNT) return;
    if(!ensure_ready(s_gpio[idx])) return;

    char p[96];
    snprintf(p, sizeof(p), "%s/gpio%d/value", gpio_root(), s_gpio[idx]);
    s_io->write(p, on ? "1" : "0");
}
