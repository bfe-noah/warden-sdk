#!/usr/bin/env python3
"""flowgen: generate mermaid flowcharts for the workflows the SDK tests.

future-features-2 asks that the test harness "produce flowcharts of every workflow
and process that it tests so a user can understand them better", each carrying its
benchmark. This emits one `docs/workflows/<name>.md` per workflow: an outcome-first
flowchart (from the modelled decision path) plus the workflow's metric: a benchmark
ns/op for the sim-modelled hardware workflows, or the MC/DC result for the Tier-1
driver workflows.

Usage:
    tools/flowgen.py [bench.json]   # bench.json = the `cargo bench` stderr trend
Reads the ns/op trend JSON if given (or sim/bench.json if present) and stamps it in.
Deterministic: same inputs -> same output (safe to run in CI and diff).
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
OUT = os.path.join(REPO, "docs", "workflows")

# Each workflow: an outcome, the mermaid body, and its metric source.
#   metric = ("bench", key)  -> ns/op from the trend json
#   metric = ("mcdc", text)  -> a Tier-1 MC/DC result line
WORKFLOWS = [
    {
        "name": "hpmcu-watchdog",
        "title": "HPMCU Watchdog",
        "outcome": "A hung A7/flared ends in a counted reset, not a dark panel.",
        "metric": ("bench", "hpmcu_tick"),
        "mermaid": """flowchart TD
  A[flared loads SCR1 fw, releases core] --> B[MCU tick]
  B --> C{mailbox magic == DISARM?}
  C -- yes --> D[disarmed: never fire]
  C -- no --> E{magic == ARMED?}
  E -- no --> B
  E -- yes --> F{beat counter advanced\\nwithin deadline?}
  F -- yes --> B
  F -- no --> G[fire CRU global reset]""",
    },
    {
        "name": "modbus-read-holding",
        "title": "Modbus RTU Round Trip",
        "outcome": "A well-formed request yields the right registers; a bad one a defined fault.",
        "metric": ("bench", "modbus_read_holding"),
        "mermaid": """flowchart TD
  A[frame in] --> B{addr == mine\\nor broadcast?}
  B -- no --> Z[ignore]
  B -- yes --> C{CRC ok?}
  C -- no --> Z
  C -- yes --> D{function code}
  D -- 0x03 read-holding --> E{range in bounds?}
  E -- no --> X[exception 0x02]
  E -- yes --> R[registers response + CRC]
  D -- unsupported --> X2[exception 0x01]""",
    },
    {
        "name": "cru-reset-ladder",
        "title": "CRU Reset Ladder",
        "outcome": "Reset cause is attributable and the boot-mode register survives a warm reset.",
        "metric": ("bench", "cru_poll"),
        "mermaid": """flowchart TD
  A[poll] --> B{global reset asserted?}
  B -- no --> A
  B -- yes --> C[record cause]
  C --> D{power-on vs warm?}
  D -- POR --> E[boot-mode cleared]
  D -- warm --> F[boot-mode preserved]""",
    },
    {
        "name": "rga-offload",
        "title": "RGA Offload",
        "outcome": "Blits go to the RGA when it succeeds, and fall back to the CPU when it doesn't.",
        "metric": ("bench", "rga_improcess"),
        "mermaid": """flowchart TD
  A[draw request] --> B{RGA compiled in\\n(#if WARDEN_USE_RGA)?}
  B -- no --> C[LVGL software draw]
  B -- yes --> D[improcess src,dst,rects IM_SYNC]
  D --> E{IM_STATUS == SUCCESS?}
  E -- yes --> F[done on RGA]
  E -- no --> C""",
    },
    {
        "name": "relay-drive",
        "title": "Relay Drive",
        "outcome": "A relay is exported transparently and driven without disturbing a held contact.",
        "metric": ("mcdc", "relays.c: 40/40 conditions, 100% MC/DC (CI-enforced)"),
        "mermaid": """flowchart TD
  A[warden_relay_set idx,on] --> B{idx < COUNT?}
  B -- no --> Z[no-op]
  B -- yes --> C{exported?}
  C -- no --> D[write export] --> E{exported now?}
  E -- no --> Z2[give up]
  E -- yes --> F
  C -- yes --> F[read direction]
  F --> G{dir == out?}
  G -- no --> H[preserve level: read value,\\nwrite high/low]
  G -- yes --> I
  H --> I[write value = on?1:0]""",
    },
    {
        "name": "freshness-contract",
        "title": "UI Freshness Contract",
        "outcome": "The UI never shows a stale number: it holds briefly, then marks unknown.",
        "metric": ("mcdc", "freshness.c: 66/66 conditions, 100% MC/DC (CI-enforced)"),
        "mermaid": """flowchart TD
  A[produce] --> B{result}
  B -- OK --> V[render value, save last]
  B -- SAME --> C{showing unknown?}
  C -- yes --> V
  C -- no --> N[no change]
  B -- UNKNOWN --> D{ever had a value?}
  D -- no --> U[render UNKNOWN mark]
  D -- yes --> E{age > max_stale?}
  E -- yes --> U
  E -- no --> N""",
    },
]


def load_bench(argv):
    path = argv[1] if len(argv) > 1 else os.path.join(REPO, "sim", "bench.json")
    out = {}
    if os.path.exists(path):
        with open(path) as f:
            for line in f:
                line = line.strip()
                if line.startswith("{") and '"bench"' in line:
                    try:
                        d = json.loads(line)
                        out[d["bench"]] = d["ns_per_op"]
                    except (ValueError, KeyError):
                        pass
    return out


def metric_line(metric, bench):
    kind, val = metric
    if kind == "bench":
        ns = bench.get(val)
        shown = f"{ns:.1f} ns/op" if ns is not None else "(run `cargo bench` to populate)"
        return f"**Benchmark** (`{val}`): {shown}"
    return f"**Coverage**: {val}"


def main():
    bench = load_bench(sys.argv)
    os.makedirs(OUT, exist_ok=True)
    index = ["# Workflow Flowcharts", "",
             "Generated by `tools/flowgen.py` from the modelled decision paths.",
             "Each is an outcome-first flowchart of a workflow the SDK tests, with its"
             " benchmark or MC/DC metric.", ""]
    for w in WORKFLOWS:
        body = (f"# {w['title']}\n\n"
                f"> **Outcome tested:** {w['outcome']}\n\n"
                f"{metric_line(w['metric'], bench)}\n\n"
                f"```mermaid\n{w['mermaid']}\n```\n")
        with open(os.path.join(OUT, w["name"] + ".md"), "w") as f:
            f.write(body)
        index.append(f"- [{w['title']}]({w['name']}.md)")
    with open(os.path.join(OUT, "README.md"), "w") as f:
        f.write("\n".join(index) + "\n")
    print(f"wrote {len(WORKFLOWS)} flowcharts + index to {os.path.relpath(OUT, REPO)}/")


if __name__ == "__main__":
    main()
