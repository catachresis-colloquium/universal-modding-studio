#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Writes its arguments after the first as one JSON array into the file named
# by its first argument (arguments are plain words).
target="$1"; shift
out="["; sep=""
for a in "$@"; do out="$out$sep\"$a\""; sep=","; done
printf '{"argv":%s]}\n' "$out" > "$target"
