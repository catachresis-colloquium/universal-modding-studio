#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Reports, into the file named by its first argument, whether the parent
# environment leaked into the child.
if [ -n "${HOME+x}${CARGO+x}${UMS_PREVIEW_LEAK+x}" ]; then printf '{"env_leaked":1}\n' > "$1"; else printf '{"env_leaked":0}\n' > "$1"; fi
