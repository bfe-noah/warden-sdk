#!/usr/bin/env python3
"""Self-hosted code-quality grade: SQALE debt ratio plus a security axis.

Reads the linter outputs collected by the CI quality job from one directory
and prints a letter grade. No external service is involved at any point;
every threshold below traces to a published number (SonarQube's default
30 min/line development cost and its maintainability grid, which
Code Climate/Qlty publish almost verbatim).

    score.py <dir> [--gate-security GRADE] [--out quality.json]

Required files in <dir> (fail-closed: a missing file is an error, so a
broken collection step can never inflate the grade):
    scc.json           scc --format json           (LOC denominator)
    clippy.jsonl       cargo clippy JSON messages, one per line
    shellcheck.json    shellcheck -f json1
    cppcheck.xml       cppcheck --xml (v2)
    lizard.csv         lizard --csv
    jscpd-report.json  jscpd --reporters json
    audit-*.json       cargo audit --json, one per crate

Exit 0 normally; 1 when --gate-security is given and the security grade is
worse; 2 on missing/unparseable input.
"""

import argparse
import csv
import glob
import json
import os
import sys
import xml.etree.ElementTree as ET

# Remediation minutes per finding severity (SQALE-style constants).
MINUTES = {"critical": 60, "major": 20, "minor": 5}
DUP_CLONE_MINUTES = 30
DEV_COST_PER_LINE = 30  # SonarQube's documented default.

# Languages that count as code for the LOC denominator.
CODE_LANGS = {"Rust", "C", "C Header", "Shell", "Python", "BASH", "Bourne Shell"}

GRADES = [(0.05, "A"), (0.10, "B"), (0.20, "C"), (0.50, "D"), (9e9, "F")]
GRADE_ORDER = "ABCDF"
# Shields palette hex codes (anybadge rejects the shields color names).
BADGE_COLORS = {"A": "#4c1", "B": "#97ca00", "C": "#dfb317", "D": "#fe7d37", "F": "#e05d44"}


def die(msg):
    print(f"FATAL: {msg}", file=sys.stderr)
    sys.exit(2)


def need(path):
    if not os.path.exists(path):
        die(f"missing required input {path}")
    return path


def load_json(path):
    with open(need(path)) as f:
        return json.load(f)


def grade_from_ratio(ratio):
    for cap, letter in GRADES:
        if ratio < cap:
            return letter
    return "F"


def parse_scc(path):
    langs = load_json(path)
    loc = sum(l["Code"] for l in langs if l["Name"] in CODE_LANGS)
    if loc <= 0:
        die("scc reports zero code lines")
    return loc


def parse_clippy(path, add):
    # Raw `cargo clippy --message-format=json` stream: one JSON object per
    # line, most of them build bookkeeping. Only lint diagnostics (messages
    # carrying a code) count.
    with open(need(path)) as f:
        lines = f.read().splitlines()
    for line in lines:
        line = line.strip()
        if not line:
            continue
        try:
            m = json.loads(line)
        except ValueError:
            die(f"unparseable clippy line: {line[:80]}")
        if m.get("reason") != "compiler-message":
            continue
        msg = m.get("message") or {}
        if not msg.get("code"):
            continue
        level = msg.get("level")
        if level == "error":
            add("clippy", "critical")
        elif level == "warning":
            add("clippy", "major")


def parse_shellcheck(path, add):
    data = load_json(path)
    for c in data.get("comments", []):
        level = c.get("level")
        sev = {"error": "critical", "warning": "major"}.get(level, "minor")
        add("shellcheck", sev)


def parse_cppcheck(path, add):
    root = ET.parse(need(path)).getroot()
    for e in root.iter("error"):
        sev = e.get("severity")
        if sev == "information":
            continue
        mapped = {"error": "critical", "warning": "major"}.get(sev, "minor")
        add("cppcheck", mapped)


def parse_lizard(path, add):
    # CSV columns: nloc, ccn, tokens, params, length, location, path, name, ...
    with open(need(path)) as f:
        rows = list(csv.reader(f))
    for row in rows:
        if len(row) < 2 or not row[1].isdigit():
            continue
        ccn = int(row[1])
        if ccn > 20:
            add("complexity", "critical")
        elif ccn > 15:
            add("complexity", "major")
        elif ccn > 10:
            add("complexity", "minor", minutes=20)


def parse_ruff(path, add, security):
    for f in load_json(path):
        code = f.get("code") or ""
        if code.startswith("S"):
            add("ruff", "major")
            security["findings"] += 1
        elif code.startswith(("E9", "F")):
            add("ruff", "major")
        else:
            add("ruff", "minor")


def parse_jscpd(path):
    stats = load_json(path)["statistics"]["total"]
    return int(stats["clones"]), float(stats["percentage"])


def parse_audits(pattern, security):
    paths = glob.glob(pattern)
    if not paths:
        die(f"no cargo-audit outputs match {pattern}")
    for p in paths:
        d = load_json(p)
        security["vulns"] += int(d["vulnerabilities"]["count"])
        security["warnings"] += sum(len(v) for v in d.get("warnings", {}).values())


def security_grade(sec):
    if sec["vulns"] >= 2:
        return "D"
    if sec["vulns"] == 1 or sec["findings"] > 0:
        return "C"
    if sec["warnings"] > 0:
        return "B"
    return "A"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("--gate-security", choices=list(GRADE_ORDER), default=None)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    d = args.dir

    counts = {}
    minutes = [0.0]

    def add(tool, sev, minutes_each=None, **kw):
        counts.setdefault(tool, {}).setdefault(sev, [0, 0.0])
        m = kw.get("minutes", minutes_each)
        if m is None:
            m = MINUTES[sev]
        counts[tool][sev][0] += 1
        counts[tool][sev][1] += m
        minutes[0] += m

    security = {"vulns": 0, "warnings": 0, "findings": 0}

    loc = parse_scc(os.path.join(d, "scc.json"))
    parse_clippy(os.path.join(d, "clippy.jsonl"), add)
    parse_shellcheck(os.path.join(d, "shellcheck.json"), add)
    parse_cppcheck(os.path.join(d, "cppcheck.xml"), add)
    parse_lizard(os.path.join(d, "lizard.csv"), add)
    parse_ruff(os.path.join(d, "ruff.json"), add, security)
    clones, dup_pct = parse_jscpd(os.path.join(d, "jscpd-report.json"))
    for _ in range(clones):
        add("duplication", "minor", minutes=DUP_CLONE_MINUTES)
    parse_audits(os.path.join(d, "audit-*.json"), security)

    ratio = minutes[0] / (loc * DEV_COST_PER_LINE)
    maint = grade_from_ratio(ratio)
    sec = security_grade(security)
    overall = max(maint, sec, key=GRADE_ORDER.index)

    report = {
        "grade": overall,
        "maintainability": {"grade": maint, "debt_ratio_pct": round(ratio * 100, 3),
                            "remediation_minutes": round(minutes[0], 1), "code_lines": loc},
        "security": {"grade": sec, **security},
        "duplication_pct": round(dup_pct, 2),
        "findings": {t: {s: {"count": v[0], "minutes": v[1]} for s, v in sevs.items()}
                     for t, sevs in counts.items()},
        "badge_color": BADGE_COLORS[overall],
    }
    if args.out:
        with open(args.out, "w") as f:
            json.dump(report, f, indent=1)
    print(f"code quality: {overall} "
          f"(maintainability {maint}, debt ratio {ratio * 100:.2f}%, "
          f"security {sec}, duplication {dup_pct:.1f}%)")
    for tool, sevs in sorted(counts.items()):
        line = ", ".join(f"{s}={v[0]}" for s, v in sorted(sevs.items()))
        print(f"  {tool}: {line}")

    if args.gate_security and GRADE_ORDER.index(sec) > GRADE_ORDER.index(args.gate_security):
        print(f"FAIL: security grade {sec} is worse than the {args.gate_security} gate",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
