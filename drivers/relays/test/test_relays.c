/* MC/DC harness for drivers/relays/relays.c.
 *
 * Two layers, one binary, so the combined run covers every decision in relays.c:
 *   1. unit tests through a FAKE relay_io — exercise the decision logic, incl.
 *      the export->node-appears path a passive tree cannot model.
 *   2. integration tests through the real sysfs backend + $WARDEN_GPIO_ROOT
 *      pointed at a scratch tree — exercise the backend's fopen/stat branches.
 */
#include "../relays.h"

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static int g_fail = 0, g_checks = 0;
#define EXPECT(cond) do { \
    g_checks++; \
    if(!(cond)) { g_fail++; fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); } \
} while(0)

/* ---------------- layer 1: fake io ---------------- */

static struct {
    bool exists_ret;
    bool export_makes_exist;   /* write(.../export) flips exists_ret true */
    bool read_line_ok;
    const char *read_line_val;
    bool read_int_ok;
    int read_int_val;
    char last_write_path[160];
    char last_write_val[32];
} fk;

static bool fk_exists(const char *p) { (void)p; return fk.exists_ret; }
static void fk_write(const char *p, const char *v) {
    snprintf(fk.last_write_path, sizeof fk.last_write_path, "%s", p);
    snprintf(fk.last_write_val, sizeof fk.last_write_val, "%s", v);
    if(fk.export_makes_exist && strstr(p, "/export")) fk.exists_ret = true;
}
static bool fk_read_line(const char *p, char *b, size_t n) {
    (void)p;
    if(!fk.read_line_ok) return false;
    snprintf(b, n, "%s", fk.read_line_val);
    return true;
}
static bool fk_read_int(const char *p, int *o) {
    (void)p;
    if(!fk.read_int_ok) return false;
    *o = fk.read_int_val;
    return true;
}
static const struct relay_io FAKE = { fk_exists, fk_write, fk_read_line, fk_read_int };

static void fk_reset(void) { memset(&fk, 0, sizeof fk); }

static void unit_tests(void)
{
    warden_relay__set_io(&FAKE);

    /* warden_relay_name: both sides of idx < COUNT */
    EXPECT(strcmp(warden_relay_name(0), "Relay 1") == 0);
    EXPECT(strcmp(warden_relay_name(WARDEN_RELAY_COUNT), "-") == 0);

    /* available: idx >= COUNT (true) */
    EXPECT(warden_relay_available(WARDEN_RELAY_COUNT) == false);

    /* ensure_ready: not exported -> export FAILS (inner if true) */
    fk_reset();
    fk.exists_ret = false; fk.export_makes_exist = false;
    EXPECT(warden_relay_available(0) == false);
    EXPECT(strstr(fk.last_write_path, "/export") != NULL); /* export was attempted */

    /* ensure_ready: not exported -> export SUCCEEDS -> direction read FAILS */
    fk_reset();
    fk.exists_ret = false; fk.export_makes_exist = true; fk.read_line_ok = false;
    EXPECT(warden_relay_available(0) == false);

    /* already exported -> direction == "out" -> no level-preserve write */
    fk_reset();
    fk.exists_ret = true; fk.read_line_ok = true; fk.read_line_val = "out\n";
    EXPECT(warden_relay_available(0) == true);

    /* exported -> direction "in" -> read_int FAILS -> cur=0 -> write "low" */
    fk_reset();
    fk.exists_ret = true; fk.read_line_ok = true; fk.read_line_val = "in\n";
    fk.read_int_ok = false;
    EXPECT(warden_relay_available(0) == true);
    EXPECT(strcmp(fk.last_write_val, "low") == 0);

    /* exported -> direction "in" -> read_int OK, cur=0 -> ternary false -> "low" */
    fk_reset();
    fk.exists_ret = true; fk.read_line_ok = true; fk.read_line_val = "in\n";
    fk.read_int_ok = true; fk.read_int_val = 0;
    EXPECT(warden_relay_available(0) == true);
    EXPECT(strcmp(fk.last_write_val, "low") == 0);

    /* exported -> direction "in" -> read_int OK, cur=1 -> ternary true -> "high" */
    fk_reset();
    fk.exists_ret = true; fk.read_line_ok = true; fk.read_line_val = "in\n";
    fk.read_int_ok = true; fk.read_int_val = 1;
    EXPECT(warden_relay_available(0) == true);
    EXPECT(strcmp(fk.last_write_val, "high") == 0);

    /* get: idx >= COUNT (true) */
    EXPECT(warden_relay_get(WARDEN_RELAY_COUNT) == false);

    /* get: not exported -> false */
    fk_reset(); fk.exists_ret = false;
    EXPECT(warden_relay_get(0) == false);

    /* get: exported, read_int FAILS -> v=0 -> v!=0 false */
    fk_reset(); fk.exists_ret = true; fk.read_int_ok = false;
    EXPECT(warden_relay_get(0) == false);

    /* get: exported, read_int OK v=1 -> v!=0 true */
    fk_reset(); fk.exists_ret = true; fk.read_int_ok = true; fk.read_int_val = 1;
    EXPECT(warden_relay_get(0) == true);

    /* get: exported, read_int OK v=0 -> v!=0 false */
    fk_reset(); fk.exists_ret = true; fk.read_int_ok = true; fk.read_int_val = 0;
    EXPECT(warden_relay_get(0) == false);

    /* set: idx >= COUNT (true) -> no-op */
    fk_reset();
    warden_relay_set(WARDEN_RELAY_COUNT, true);
    EXPECT(fk.last_write_path[0] == '\0');

    /* set: ensure_ready FAILS -> return before writing value */
    fk_reset(); fk.exists_ret = false; fk.export_makes_exist = false;
    warden_relay_set(0, true);
    EXPECT(strstr(fk.last_write_path, "/value") == NULL);

    /* set: ensure_ready OK (exported, "out"), on=true -> write "1" */
    fk_reset(); fk.exists_ret = true; fk.read_line_ok = true; fk.read_line_val = "out\n";
    warden_relay_set(0, true);
    EXPECT(strcmp(fk.last_write_val, "1") == 0);

    /* set: on=false -> write "0" */
    fk_reset(); fk.exists_ret = true; fk.read_line_ok = true; fk.read_line_val = "out\n";
    warden_relay_set(0, false);
    EXPECT(strcmp(fk.last_write_val, "0") == 0);
}

/* ---------------- layer 2: real sysfs backend over a scratch tree ---------------- */

static char g_root[128];

static void wr(const char *rel, const char *content) {
    char p[256]; snprintf(p, sizeof p, "%s/%s", g_root, rel);
    FILE *f = fopen(p, "w"); if(f) { fputs(content, f); fclose(f); }
}
static void mkgpio(int n) {
    char p[256]; snprintf(p, sizeof p, "%s/gpio%d", g_root, n); mkdir(p, 0777);
}

static void integration_tests(void)
{
    char tmpl[] = "/tmp/warden-relays-XXXXXX";
    char *d = mkdtemp(tmpl);
    assert(d);
    snprintf(g_root, sizeof g_root, "%s", d);
    setenv("WARDEN_GPIO_ROOT", g_root, 1);
    warden_relay__set_io(NULL);   /* restore the real sysfs backend */

    /* empty $WARDEN_GPIO_ROOT -> gpio_root() falls back to the default (covers the
       `*r` false arm of `r && *r`). The default /sys path is absent on the host,
       so the call simply reports unavailable. */
    setenv("WARDEN_GPIO_ROOT", "", 1);
    (void)warden_relay_available(0);
    setenv("WARDEN_GPIO_ROOT", g_root, 1);

    /* sysfs_exists false + sysfs_write fopen FAIL: root has no /export parent issue;
       use a nonexistent root so export write and stat both fail cleanly. */
    setenv("WARDEN_GPIO_ROOT", "/nonexistent-warden-root/xyz", 1);
    EXPECT(warden_relay_available(0) == false);   /* exists=false, write(export) fopen fails */
    setenv("WARDEN_GPIO_ROOT", g_root, 1);

    /* gpio32 exists, direction file ABSENT -> sysfs_read_line fopen fail */
    mkgpio(32);
    EXPECT(warden_relay_available(0) == false);

    /* direction EMPTY -> fgets returns NULL -> read_line false */
    wr("gpio32/direction", "");
    EXPECT(warden_relay_available(0) == false);

    /* direction "out" -> read_line ok, skip block -> available true (sysfs_write success path
       is exercised by set below); also value ABSENT -> get: read_int fopen fail -> v=0 */
    wr("gpio32/direction", "out\n");
    EXPECT(warden_relay_available(0) == true);
    EXPECT(warden_relay_get(0) == false);          /* value file absent: read_int fopen fail */

    /* value non-numeric -> fscanf != 1 -> read_int false */
    wr("gpio32/value", "xyz\n");
    EXPECT(warden_relay_get(0) == false);

    /* value "1" -> read_int ok -> get true; and set(0,true) -> sysfs_write success */
    wr("gpio32/value", "1\n");
    EXPECT(warden_relay_get(0) == true);
    warden_relay_set(0, false);
    EXPECT(warden_relay_get(0) == false);          /* wrote "0" over the value file */

    /* direction "in" with a real value -> exercises the level-preserve write via sysfs */
    mkgpio(33);
    wr("gpio33/direction", "in\n");
    wr("gpio33/value", "1\n");
    EXPECT(warden_relay_available(1) == true);      /* reads value=1, writes direction "high" */
}

int main(void)
{
    unit_tests();
    integration_tests();
    fprintf(stderr, "%d checks, %d failures\n", g_checks, g_fail);
    return g_fail ? 1 : 0;
}
