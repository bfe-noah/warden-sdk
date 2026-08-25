# MC/DC unit harness for drivers/freshness/freshness.c (the UI stale-number guard).
#
#   make check   — build, run, FAIL unless freshness.c hits 100% MC/DC + all checks.
#   make report  — per-condition gcov annotation.
#   make clean
#
# FRESH_MAX is forced to 2 so the "binding table full -> NULL" path is reachable
# with two binds (the production default is 96). Requires gcc >= 14.
CC      ?= gcc
GCOV    ?= gcov
CFLAGS  := -O0 -g -Wall -Wextra -I.. -DFRESH_MAX=2
COVFLAGS := --coverage -fcondition-coverage
BUILD   := build

.PHONY: check report clean
.DEFAULT_GOAL := check

$(BUILD):
	@mkdir -p $(BUILD)

$(BUILD)/test: test_freshness.c ../freshness.c ../freshness.h | $(BUILD)
	@$(CC) $(CFLAGS) $(COVFLAGS) -c ../freshness.c -o $(BUILD)/freshness.o
	@$(CC) $(CFLAGS) -c test_freshness.c -o $(BUILD)/test_freshness.o
	@$(CC) $(COVFLAGS) $(BUILD)/freshness.o $(BUILD)/test_freshness.o -o $(BUILD)/test

check: $(BUILD)/test
	@echo "== running freshness MC/DC harness =="
	@rm -f $(BUILD)/freshness.gcda
	@$(BUILD)/test; echo $$? > $(BUILD)/test.rc
	@echo
	@echo "== MC/DC (condition) coverage of freshness.c =="
	@$(GCOV) --conditions --branch-probabilities -o $(BUILD) ../freshness.c >$(BUILD)/gcov.log 2>&1 || true
	@mv -f *.gcov $(BUILD)/ 2>/dev/null || true
	@bash enforce-mcdc.sh $(BUILD)/gcov.log $(BUILD)/freshness.c.gcov $(BUILD)/test.rc

report: check
	@grep -nE "condition.*not covered|conditions covered" $(BUILD)/freshness.c.gcov || true

clean:
	@rm -rf $(BUILD)
