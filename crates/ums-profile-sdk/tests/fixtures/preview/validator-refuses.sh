#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Refuses with a named rule and a field path, as exit 1.
while IFS= read -r line; do :; done
printf '{"path":"/ticks/0/conation/selected","rule":"UnexplainedSelection"}\n'
exit 1
