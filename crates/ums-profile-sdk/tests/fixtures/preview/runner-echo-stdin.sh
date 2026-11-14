#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Copies its stdin (the preview request) into the file named by its first
# argument.
target="$1"
while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "$target"
