# MC/DC unit harness for drivers/relays/relays.c.
#
#   make check   — build, run, and FAIL unless relays.c hits 100% MC/DC
#                  (condition) coverage and every unit check passes.
#   make report  — per-condition gcov annotation (build/relays.c.gcov).
#   make clean
#
# Requires gcc >= 14 (for -fcondition-coverage) and its matching gcov.
CC      ?= gcc
GCOV    ?= gcov
CFLAGS  := -O0 -g -Wall -Wextra -I..
COVFLAGS := --coverage -fcondition-coverage
BUILD   := build

.PHONY: check report clean
.DEFAULT_GOAL := check

$(BUILD):
	@mkdir -p $(BUILD)

# relays.c carries the coverage flags; the test driver is plain.
$(BUILD)/test: test_relays.c ../relays.c ../relays.h | $(BUILD)
	@$(CC) $(CFLAGS) $(COVFLAGS) -c ../relays.c -o $(BUILD)/relays.o
	@$(CC) $(CFLAGS) -c test_relays.c -o $(BUILD)/test_relays.o
	@$(CC) $(COVFLAGS) $(BUILD)/relays.o $(BUILD)/test_relays.o -o $(BUILD)/test

check: $(BUILD)/test
	@echo "== running relays MC/DC harness =="
	@rm -f $(BUILD)/relays.gcda
	@$(BUILD)/test; echo $$? > $(BUILD)/test.rc
	@echo
	@echo "== MC/DC (condition) coverage of relays.c =="
	@$(GCOV) --conditions --branch-probabilities -o $(BUILD) ../relays.c >$(BUILD)/gcov.log 2>&1 || true
	@mv -f *.gcov $(BUILD)/ 2>/dev/null || true
	@bash ../../enforce-mcdc.sh $(BUILD)/gcov.log $(BUILD)/relays.c.gcov $(BUILD)/test.rc

report: check
	@grep -nE "condition.*not covered|conditions covered" $(BUILD)/relays.c.gcov || true

clean:
	@rm -rf $(BUILD)
