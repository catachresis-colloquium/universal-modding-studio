#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Refuses with two faults in the canonical one-line form of enaction-validate
# (exit 1). The adapter reports the first and counts the rest.
while IFS= read -r line; do :; done
printf '{"faults":[{"detail":"engage is neither first nor the incumbent","path":"mind_trace.ticks[0].conation.selected","rule":"UnexplainedSelection"},{"detail":"tick 3 follows tick 4","path":"mind_trace.ticks[1].tick","rule":"NonMonotonicTick"}]}\n'
printf 'UnexplainedSelection\tmind_trace.ticks[0].conation.selected\tengage is neither first nor the incumbent\n' >&2
exit 1
