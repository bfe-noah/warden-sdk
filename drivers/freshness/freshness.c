/*
 * The UI Freshness Contract engine (ADR 0004) — core, LVGL-free.
 * See freshness.h for the contract. LVGL binding lives in freshness_lv.c.
 */
#include "freshness.h"

#include <stdio.h>
#include <string.h>

/* The unknown mark the engine renders when a source cannot be evaluated (from
 * freshness.h so the LVGL layer and tests share the exact literal). */
#define FRESH_UNKNOWN_MARK WARDEN_FRESH_UNKNOWN_MARK

/* Max simultaneous live bindings. Bindings belong to visible pages; the whole
 * navigable set of a screen is small, so this is generous. A full table drops
 * the binding (returns NULL) rather than silently overflowing — the LVGL layer
 * turns that into a visible fault, never a stale value. */
#ifndef FRESH_MAX
#define FRESH_MAX 96
#endif

#define FRESH_BUFSZ 64

struct warden_fresh {
    void *                    page;
    warden_fresh_produce_cb   produce;
    void *                    ud;
    uint32_t                  max_stale_ms;
    const char *              source;
    warden_fresh_render_cb    render;
    void *                    widget;
    uint32_t                  last_ok_ms;   /* time of last OK/SAME produce   */
    char                      last[FRESH_BUFSZ]; /* last good value string    */
    bool                      ever_ok;
    bool                      showing_unknown;
    bool                      visible;
    bool                      used;
};

/* A fixed table scanned in full: bindings are torn down all at once by
 * warden_fresh_reset (like the screen timers), never individually, so a running
 * high-water bound would only hide the free-slot arms from tests without saving
 * real work — the visible set per screen is a handful. */
static struct warden_fresh s_vals[FRESH_MAX];

warden_fresh_render_t warden_fresh_decide(warden_fresh_result_t produced,
                                          bool ever_ok, bool showing_unknown,
                                          uint32_t age_ms, uint32_t max_stale_ms)
{
    switch(produced) {
        case FRESH_OK:
            return FRESH_RENDER_VALUE;
        case FRESH_SAME:
            /* Unchanged and fresh: normally nothing to do. But if the widget is
             * currently showing UNKNOWN (it went stale), an unchanged value
             * still has to be repainted to clear the mark. */
            return showing_unknown ? FRESH_RENDER_VALUE : FRESH_RENDER_NOCHANGE;
        case FRESH_UNKNOWN:
        default:
            /* Never had a value, or the last good value is now older than its
             * budget: stop asserting a confident number. Otherwise tolerate a
             * brief blip and hold the last value until the budget expires. */
            if(!ever_ok) return FRESH_RENDER_UNKNOWN;
            if(age_ms > max_stale_ms) return FRESH_RENDER_UNKNOWN;
            return FRESH_RENDER_NOCHANGE;
    }
}

static void refresh_one(struct warden_fresh *v, uint32_t now)
{
    char buf[FRESH_BUFSZ];
    buf[0] = '\0';
    warden_fresh_result_t r = v->produce(buf, sizeof buf, v->ud);

    warden_fresh_render_t what = warden_fresh_decide(
        r, v->ever_ok, v->showing_unknown, now - v->last_ok_ms, v->max_stale_ms);

    if(r == FRESH_OK) {
        /* Keep the last good string so a later SAME-recovery can repaint it.
         * snprintf truncates and null-terminates; buf and last are both
         * FRESH_BUFSZ, so this cannot overflow. */
        snprintf(v->last, sizeof v->last, "%s", buf);
    }
    if(r == FRESH_OK || r == FRESH_SAME) {
        v->last_ok_ms = now;
        v->ever_ok = true;
    }

    switch(what) {
        case FRESH_RENDER_VALUE:
            v->render(v->widget, FRESH_RENDER_VALUE,
                      (r == FRESH_OK) ? buf : v->last, v->ud);
            v->showing_unknown = false;
            break;
        case FRESH_RENDER_UNKNOWN:
            v->render(v->widget, FRESH_RENDER_UNKNOWN, FRESH_UNKNOWN_MARK, v->ud);
            v->showing_unknown = true;
            break;
        case FRESH_RENDER_NOCHANGE:
            break;
    }
}

warden_fresh_t *warden_fresh_bind(void *page, warden_fresh_produce_cb produce,
                                  void *ud, uint32_t max_stale_ms,
                                  const char *source,
                                  warden_fresh_render_cb render, void *widget)
{
    if(!produce || !render) return NULL;
    for(uint32_t i = 0; i < FRESH_MAX; i++) {
        if(s_vals[i].used) continue;
        struct warden_fresh *v = &s_vals[i];
        memset(v, 0, sizeof *v);
        v->page = page;
        v->produce = produce;
        v->ud = ud;
        v->max_stale_ms = max_stale_ms;
        v->source = source;
        v->render = render;
        v->widget = widget;
        v->visible = true; /* bound while building the page that's about to show */
        v->used = true;
        return v;
    }
    return NULL; /* table full — caller surfaces a fault, never a stale value */
}

void warden_fresh_set_visible(void *page, bool visible)
{
    for(uint32_t i = 0; i < FRESH_MAX; i++) {
        if(s_vals[i].used && s_vals[i].page == page) s_vals[i].visible = visible;
    }
}

void warden_fresh_page_show(void *page, uint32_t now_ms)
{
    for(uint32_t i = 0; i < FRESH_MAX; i++) {
        struct warden_fresh *v = &s_vals[i];
        if(v->used && v->page == page) {
            v->visible = true;
            refresh_one(v, now_ms);
        }
    }
}

void warden_fresh_tick(uint32_t now_ms)
{
    for(uint32_t i = 0; i < FRESH_MAX; i++) {
        if(s_vals[i].used && s_vals[i].visible) refresh_one(&s_vals[i], now_ms);
    }
}

void warden_fresh_invalidate(const char *source, uint32_t now_ms)
{
    if(!source) return;
    for(uint32_t i = 0; i < FRESH_MAX; i++) {
        struct warden_fresh *v = &s_vals[i];
        if(v->used && v->visible && v->source && strcmp(v->source, source) == 0) {
            refresh_one(v, now_ms);
        }
    }
}

void warden_fresh_reset(void)
{
    memset(s_vals, 0, sizeof s_vals);
}

uint32_t warden_fresh_count(void)
{
    uint32_t n = 0;
    for(uint32_t i = 0; i < FRESH_MAX; i++) if(s_vals[i].used) n++;
    return n;
}

uint32_t warden_fresh_min_budget_ms(void)
{
    uint32_t best = 0;
    for(uint32_t i = 0; i < FRESH_MAX; i++) {
        struct warden_fresh *v = &s_vals[i];
        if(v->used && v->visible && (best == 0 || v->max_stale_ms < best)) {
            best = v->max_stale_ms;
        }
    }
    return best;
}
