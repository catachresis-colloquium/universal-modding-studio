#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Accepts only input that carries a runner-output package.
IFS= read -r first || exit 2
case "$first" in
  *\"package\"*) while IFS= read -r line; do :; done; exit 0 ;;
esac
printf '{"rule":"NoRunnerOutput","path":""}\n'
exit 1
