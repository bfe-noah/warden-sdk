#!/usr/bin/env bash
# Gate: fail unless every unit check passed AND relays.c reached 100% MC/DC
# (condition) coverage with no uncovered executable lines.
#   $1 = gcov stdout log   $2 = relays.c.gcov   $3 = test exit-code file
set -uo pipefail
LOG="$1"; GCOV="$2"; RCFILE="$3"
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

# Summary lines for relays.c from gcov stdout. Match the exact file so
# 'test_relays.c' (which also contains "relays.c") is NOT picked up.
FMATCH="File '([^']*/)?relays[.]c'"
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
	echo "RESULT: uncovered executable lines in relays.c:"
	echo "$uncov_lines" | sed 's/^/    /'
	rc=1
fi

if ! echo "$cond_line" | grep -q "100.00%"; then
	echo "RESULT: condition coverage is below 100%"
	rc=1
fi

[ "$rc" = "0" ] && echo "RESULT: 100% MC/DC + all checks green ✓"
exit "$rc"
