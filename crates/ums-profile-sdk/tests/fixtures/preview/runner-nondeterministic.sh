#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Nondeterministic: the runner-output file carries the process id, which
# differs between the two runs. Its stdout is the same in both runs.
while [ $# -gt 0 ]; do
  case "$1" in
    --emit-epistemic) epi="$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '{"run":%s}\n' "$$" > "$epi"
printf 'summary\n'
