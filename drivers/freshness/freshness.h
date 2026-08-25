/*
 * The UI Freshness Contract engine (ADR 0004) — core, LVGL-free.
 *
 * The panel is read on-site to judge whether hardware is healthy, so a silently
 * *stale* number is worse than a missing one: a stale IP or hashrate reads as
 * ground truth and sends a technician the wrong way. This engine is the only
 * sanctioned way to show a live value. It guarantees a bound value is refreshed
 * (a) the instant its page becomes visible, (b) periodically while visible
 * within a declared max-staleness, and (c) promptly when a declared source
 * changes — and it renders a value whose source cannot be evaluated as an
 * explicit UNKNOWN, never as its confident last-known number.
 *
 * This header is deliberately LVGL-free so the engine and every producer are
 * unit-testable headlessly. The LVGL label convenience lives in freshness_lv.h.
 */
#ifndef WARDEN_FRESHNESS_H
#define WARDEN_FRESHNESS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The mark shown when a value's source cannot be evaluated (em dash). The LVGL
 * layer additionally dims the widget. Public so the label wrapper and tests can
 * reference the same literal. */
#define WARDEN_FRESH_UNKNOWN_MARK "\xE2\x80\x94"

/* What a producer reports after being asked to produce the current value. */
typedef enum {
    FRESH_OK = 0,   /* wrote the current value into buf                     */
    FRESH_UNKNOWN,  /* source unavailable — no value can be produced now    */
    FRESH_SAME,     /* source read fine; value unchanged (cheap re-render)  */
} warden_fresh_result_t;

/* Produce the current value of a live datum into buf. Pure w.r.t. LVGL: a plain
 * function of the model behind `ud`, which is exactly why it is the test seam. */
typedef warden_fresh_result_t (*warden_fresh_produce_cb)(char *buf, size_t n,
                                                         void *ud);

/* What the engine decided the widget should show this cycle. */
typedef enum {
    FRESH_RENDER_VALUE = 0, /* show the produced/last-good value */
    FRESH_RENDER_UNKNOWN,   /* show the explicit-unknown mark ("—", dimmed) */
    FRESH_RENDER_NOCHANGE,  /* leave the widget exactly as it is */
} warden_fresh_render_t;

/* The one pure decision at the heart of the contract — no state, no I/O, no
 * LVGL, so every branch is unit-testable. `showing_unknown` is whether the
 * widget is currently displaying the UNKNOWN mark (so recovery from a stale
 * blip re-renders the value even when the producer reports it unchanged). */
warden_fresh_render_t warden_fresh_decide(warden_fresh_result_t produced,
                                          bool ever_ok, bool showing_unknown,
                                          uint32_t age_ms, uint32_t max_stale_ms);

/* The widget-render seam: the engine calls this to actually update a widget.
 * `buf` is valid only for FRESH_RENDER_VALUE. LVGL lives behind this callback. */
typedef void (*warden_fresh_render_cb)(void *widget, warden_fresh_render_t what,
                                       const char *buf, void *ud);

typedef struct warden_fresh warden_fresh_t;

/* Bind a producer+widget to a page. `source` (may be NULL) is a named change
 * channel for warden_fresh_invalidate. Returns NULL if the table is full. */
warden_fresh_t *warden_fresh_bind(void *page, warden_fresh_produce_cb produce,
                                  void *ud, uint32_t max_stale_ms,
                                  const char *source,
                                  warden_fresh_render_cb render, void *widget);

/* A page became visible/hidden (tiles via notify_active, subnav leaves via
 * on_open). Hidden pages are skipped by the periodic tick. */
void warden_fresh_set_visible(void *page, bool visible);

/* Refresh every bound value on `page` right now (the on-show guarantee). Also
 * marks the page visible. */
void warden_fresh_page_show(void *page, uint32_t now_ms);

/* The shared periodic tick: refresh every currently-visible bound value. */
void warden_fresh_tick(uint32_t now_ms);

/* A producer of change fired: refresh every visible value bound to `source`. */
void warden_fresh_invalidate(const char *source, uint32_t now_ms);

/* Drop all bindings — called on a theme/screen rebuild, like the screen timers. */
void warden_fresh_reset(void);

/* Number of live bindings (introspection / tests). */
uint32_t warden_fresh_count(void);

/* Smallest max_stale_ms among visible bindings, or 0 if none — lets the LVGL
 * layer size the shared tick to the tightest budget actually on screen. */
uint32_t warden_fresh_min_budget_ms(void);

#endif /* WARDEN_FRESHNESS_H */
