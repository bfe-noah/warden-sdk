#!/usr/bin/env bash
# Shared Tier-1 MC/DC gate for every driver harness. Each driver's test/Makefile
# invokes it as:
#   bash ../../enforce-mcdc.sh <gcov.log> <build>/<name>.c.gcov <test.rc>
# Fails unless every unit check passed AND <name>.c reached 100% MC/DC (condition)
# coverage with no uncovered executable lines. The driver name is derived from the
# .gcov filename so this one script serves all drivers (no per-driver copies to keep
# in sync).
#   $1 = gcov stdout log   $2 = <name>.c.gcov   $3 = test exit-code file
set -uo pipefail
LOG="$1"; GCOV="$2"; RCFILE="$3"
base="$(basename "$GCOV")"; base="${base%.gcov}"   # e.g. freshness.c
name="${base%.c}"                                   # e.g. freshness
rc=0

testrc="$(cat "$RCFILE" 2>/dev/null || echo 1)"
if [ "$testrc" != "0" ]; then
	echo "RESULT: unit checks FAILED (test exit $testrc)"; rc=1
else
	echo "RESULT: all unit checks passed"
fi

if [ ! -f "$GCOV" ]; then
	echo "RESULT: no coverage file ($GCOV) produced"; exit 1
fi

notcov="$(grep -nE "condition[s]? .*not covered" "$GCOV" || true)"
uncov_lines="$(grep -nE "^ +#####:" "$GCOV" || true)"

# Summary lines for <name>.c from gcov stdout. Match the exact file so the test
# harness translation unit (test_<name>.c, which also contains "<name>.c") is NOT
# picked up.
FMATCH="File '([^']*/)?${name}[.]c'"
cond_line="$(awk -v patt="$FMATCH" '$0 ~ patt {f=1} f&&/Condition outcomes covered:/{print; f=0}' "$LOG")"
line_line="$(awk -v patt="$FMATCH" '$0 ~ patt {f=1} f&&/Lines executed:/{print; f=0}' "$LOG")"
echo "  ${line_line:-Lines executed: (n/a)}"
echo "  ${cond_line:-Condition outcomes covered: (n/a)}"

if [ -n "$notcov" ]; then
	echo "RESULT: MC/DC gaps (conditions not covered):"
	echo "$notcov" | sed 's/^/    /'
	rc=1
fi
if [ -n "$uncov_lines" ]; then
	echo "RESULT: uncovered executable lines in ${name}.c:"
	echo "$uncov_lines" | sed 's/^/    /'
	rc=1
fi

if ! echo "$cond_line" | grep -q "100.00%"; then
	echo "RESULT: condition coverage is below 100%"
	rc=1
fi

[ "$rc" = "0" ] && echo "RESULT: 100% MC/DC + all checks green ✓"
exit "$rc"
