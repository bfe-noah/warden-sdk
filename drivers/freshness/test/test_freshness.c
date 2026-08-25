/* MC/DC harness for drivers/freshness/freshness.c (built with -DFRESH_MAX=2).
 *
 * freshness.c is pure logic with produce/render callbacks — no hardware seam
 * needed, the callbacks ARE the seam. We drive the decision function directly and
 * the bind/tick/invalidate state machine through programmable fakes, covering
 * every decision (incl. the compound `used && visible`, `!produce || !render`,
 * `used && visible && source && strcmp==0`, `best==0 || max_stale<best`).
 */
#include "../freshness.h"

#include <stdio.h>
#include <string.h>

static int g_fail = 0, g_checks = 0;
#define EXPECT(c) do { g_checks++; if(!(c)) { g_fail++; \
    fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #c); } } while(0)

/* --- programmable produce/render fakes --- */
static warden_fresh_result_t g_prod_ret;
static const char *g_prod_val;
static warden_fresh_result_t fake_produce(char *buf, size_t n, void *ud) {
    (void)ud;
    if(g_prod_val) snprintf(buf, n, "%s", g_prod_val);
    return g_prod_ret;
}
static int g_render_calls;
static warden_fresh_render_t g_render_what;
static char g_render_buf[64];
static void fake_render(void *widget, warden_fresh_render_t what, const char *buf, void *ud) {
    (void)widget; (void)ud;
    g_render_calls++; g_render_what = what;
    snprintf(g_render_buf, sizeof g_render_buf, "%s", buf ? buf : "");
}
static void set_produce(warden_fresh_result_t r, const char *v) { g_prod_ret = r; g_prod_val = v; }
static void reset_render(void) { g_render_calls = 0; g_render_what = FRESH_RENDER_NOCHANGE; g_render_buf[0] = 0; }

/* --- 1. the pure decision function: every switch arm + inner condition --- */
static void test_decide(void) {
    EXPECT(warden_fresh_decide(FRESH_OK, false, false, 0, 100) == FRESH_RENDER_VALUE);
    /* SAME: showing_unknown both ways */
    EXPECT(warden_fresh_decide(FRESH_SAME, true, true,  0, 100) == FRESH_RENDER_VALUE);
    EXPECT(warden_fresh_decide(FRESH_SAME, true, false, 0, 100) == FRESH_RENDER_NOCHANGE);
    /* UNKNOWN: !ever_ok true; then age>max true; then age<=max */
    EXPECT(warden_fresh_decide(FRESH_UNKNOWN, false, false, 0,   100) == FRESH_RENDER_UNKNOWN);
    EXPECT(warden_fresh_decide(FRESH_UNKNOWN, true,  false, 200, 100) == FRESH_RENDER_UNKNOWN);
    EXPECT(warden_fresh_decide(FRESH_UNKNOWN, true,  false, 50,  100) == FRESH_RENDER_NOCHANGE);
    /* boundary: age == max_stale is NOT stale (guards a `>`->`>=` regression that
     * MC/DC alone would not catch — both outcomes are already covered above). */
    EXPECT(warden_fresh_decide(FRESH_UNKNOWN, true,  false, 100, 100) == FRESH_RENDER_NOCHANGE);
}

/* --- 2. bind: !produce, !render, valid, and table-full (FRESH_MAX=2) --- */
static void test_bind(void) {
    warden_fresh_reset();
    EXPECT(warden_fresh_bind((void*)1, NULL, NULL, 100, "s", fake_render, (void*)9) == NULL); /* !produce */
    EXPECT(warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", NULL, (void*)9) == NULL); /* !render */
    warden_fresh_t *a = warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", fake_render, (void*)9);
    warden_fresh_t *b = warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", fake_render, (void*)9);
    EXPECT(a && b);                         /* both slots taken */
    EXPECT(warden_fresh_count() == 2);
    EXPECT(warden_fresh_bind((void*)1, fake_produce, NULL, 100, "t", fake_render, (void*)9) == NULL); /* full */
}

/* --- 3. refresh_one via tick: OK / SAME(recover) / UNKNOWN-hold / UNKNOWN-stale --- */
static void test_refresh_paths(void) {
    warden_fresh_reset();
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", fake_render, (void*)9);

    /* OK at t=0 -> render VALUE(buf), last saved, ever_ok set */
    set_produce(FRESH_OK, "42"); reset_render();
    warden_fresh_tick(0);
    EXPECT(g_render_calls == 1 && g_render_what == FRESH_RENDER_VALUE && strcmp(g_render_buf, "42") == 0);

    /* UNKNOWN, ever_ok, age<=max -> NOCHANGE (r==OK||SAME both false; what NOCHANGE) */
    set_produce(FRESH_UNKNOWN, NULL); reset_render();
    warden_fresh_tick(50);
    EXPECT(g_render_calls == 0);

    /* UNKNOWN, age>max -> UNKNOWN render, showing_unknown=true */
    reset_render();
    warden_fresh_tick(500);
    EXPECT(g_render_calls == 1 && g_render_what == FRESH_RENDER_UNKNOWN);

    /* SAME while showing_unknown -> VALUE render of v->last (r!=OK ternary false-arm) */
    set_produce(FRESH_SAME, NULL); reset_render();
    warden_fresh_tick(520);
    EXPECT(g_render_calls == 1 && g_render_what == FRESH_RENDER_VALUE && strcmp(g_render_buf, "42") == 0);
}

/* --- 4. set_visible / page_show / tick(used&&visible) / count(used) --- */
static void test_visibility(void) {
    warden_fresh_reset();
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", fake_render, (void*)9); /* page 1 */
    warden_fresh_bind((void*)2, fake_produce, NULL, 100, "s", fake_render, (void*)9); /* page 2 */

    /* set_visible: page match vs no-match (used true both; page==page T/F) */
    warden_fresh_set_visible((void*)1, false);   /* page 1 hidden */
    set_produce(FRESH_OK, "7"); reset_render();
    warden_fresh_tick(0);                          /* only page-2 (visible) refreshes */
    EXPECT(g_render_calls == 1);

    /* page_show forces visible + refreshes just that page */
    reset_render();
    warden_fresh_page_show((void*)1, 1);
    EXPECT(g_render_calls == 1);

    /* count sees used slots; an unused slot exercises the `used` false arm in the
     * count/tick loops */
    EXPECT(warden_fresh_count() == 2);
}

/* --- 5. invalidate: !source; source match/no-match; and a NULL-source binding --- */
static void test_invalidate(void) {
    warden_fresh_reset();
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, "alpha", fake_render, (void*)9);
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, NULL,    fake_render, (void*)9); /* source NULL */

    warden_fresh_invalidate(NULL, 0);              /* !source -> early return */
    set_produce(FRESH_OK, "1"); reset_render();
    warden_fresh_invalidate("beta", 0);            /* no source matches -> no refresh */
    EXPECT(g_render_calls == 0);
    reset_render();
    warden_fresh_invalidate("alpha", 0);           /* matches the first binding only */
    EXPECT(g_render_calls == 1);
}

/* --- 6. min_budget: unseen first, then max_stale<best true and false --- */
static void test_min_budget(void) {
    warden_fresh_reset();
    EXPECT(warden_fresh_min_budget_ms() == 0);     /* nothing bound */
    /* regression (correctness review): a zero-tolerance binding (max_stale=0) must
     * win the minimum, not be mistaken for the "nothing scanned" sentinel and
     * widened to a looser neighbour's budget. */
    warden_fresh_bind((void*)1, fake_produce, NULL, 0,   "s", fake_render, (void*)9);
    warden_fresh_bind((void*)1, fake_produce, NULL, 300, "s", fake_render, (void*)9);
    EXPECT(warden_fresh_min_budget_ms() == 0);
    warden_fresh_reset();
    EXPECT(warden_fresh_min_budget_ms() == 0);     /* nothing bound (post-reset) */
    warden_fresh_bind((void*)1, fake_produce, NULL, 300, "s", fake_render, (void*)9); /* best=0->300 */
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", fake_render, (void*)9); /* 100<300 -> 100 */
    EXPECT(warden_fresh_min_budget_ms() == 100);
    /* a third can't bind (full at 2); rebind fresh with the larger-first order so the
     * `max_stale < best` FALSE arm (200 !< 100) is taken */
    warden_fresh_reset();
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", fake_render, (void*)9);
    warden_fresh_bind((void*)1, fake_produce, NULL, 200, "s", fake_render, (void*)9); /* 200<100 false */
    EXPECT(warden_fresh_min_budget_ms() == 100);
    /* a hidden binding exercises min_budget's `visible` false arm */
    warden_fresh_set_visible((void*)1, false);
    EXPECT(warden_fresh_min_budget_ms() == 0);
}

/* --- 7. the `used`/`visible` FALSE arms of the scan loops: bind ONE (leaving a
 *        slot unused) and hide it, so set_visible/page_show/invalidate/count each
 *        see an unused and an invisible slot. --- */
static void test_false_arms(void) {
    warden_fresh_reset();
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, "x", fake_render, (void*)9); /* slot0 used; slot1 unused */

    EXPECT(warden_fresh_count() == 1);            /* count: slot1 used==false */
    warden_fresh_set_visible((void*)2, false);    /* set_visible: slot0 page-mismatch, slot1 used==false */
    warden_fresh_page_show((void*)2, 0);          /* page_show: slot1 used==false */

    set_produce(FRESH_OK, "1"); reset_render();
    warden_fresh_invalidate("x", 0);              /* invalidate: slot0 matches, slot1 used==false */
    EXPECT(g_render_calls == 1);

    warden_fresh_set_visible((void*)1, false);    /* hide the used binding */
    reset_render();
    warden_fresh_invalidate("x", 0);              /* invalidate: slot0 used but visible==false */
    EXPECT(g_render_calls == 0);
}

/* --- 8. clock wraparound: a uint32 ms counter wraps every ~49.7 days, so a tick
 *        can arrive with now_ms < last_ok_ms. The unsigned `now - last_ok_ms` then
 *        underflows to a huge age; the contract must fail SAFE to UNKNOWN, never
 *        assert the last value as if still fresh. --- */
static void test_wraparound(void) {
    warden_fresh_reset();
    warden_fresh_bind((void*)1, fake_produce, NULL, 100, "s", fake_render, (void*)9);
    set_produce(FRESH_OK, "9"); reset_render();
    warden_fresh_tick(1000);                       /* last_ok_ms = 1000, ever_ok */
    EXPECT(g_render_calls == 1 && g_render_what == FRESH_RENDER_VALUE);
    /* now wraps behind last_ok_ms: age underflows -> treated as stale -> UNKNOWN */
    set_produce(FRESH_UNKNOWN, NULL); reset_render();
    warden_fresh_tick(10);
    EXPECT(g_render_calls == 1 && g_render_what == FRESH_RENDER_UNKNOWN);
}

int main(void) {
    test_decide();
    test_bind();
    test_refresh_paths();
    test_visibility();
    test_invalidate();
    test_min_budget();
    test_false_arms();
    test_wraparound();
    fprintf(stderr, "%d checks, %d failures\n", g_checks, g_fail);
    return g_fail ? 1 : 0;
}
