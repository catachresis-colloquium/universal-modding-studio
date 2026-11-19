#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/explain_golden.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Writes the same documents as ../preview/runner-deterministic.sh, with the
# runner output in the kernel contract 2.0 form (enaction-runner-output/2):
# --emit-epistemic PATH for the runner output, optional --emit-mind-trace
# PATH for the mind trace. Other known arguments are ignored.
epi=""; mind=""
while [ $# -gt 0 ]; do
  case "$1" in
    --emit-epistemic) [ $# -ge 2 ] || exit 2; epi="$2"; shift 2 ;;
    --emit-mind-trace) [ $# -ge 2 ] || exit 2; mind="$2"; shift 2 ;;
    --profile|--ticks|--seed) shift 2 ;;
    *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done
if [ -z "$epi" ]; then printf -- '--emit-epistemic needs a path\n' >&2; exit 2; fi
printf 'kernel      : stub\n'
printf '%s\n' '{"contract":"enaction-runner-output/2","package":{"beliefs":[{"bearer":"agent-a","belief_id":1,"confidence":{"belief":8000,"plausibility":9500},"proposition":"door is north","source_events":[17],"status":"believed","valid_from":40}],"events":[{"confidence":9000,"event_id":17,"kind":"observation","observer":"agent-a","proposition":"door is north","provenance":{"origin":"sight"},"sequence":17,"subject":{"agent":"agent-a"},"tick":40}],"manifest":{"digest":"0000000000000000000000000000000000000000000000000000000000000000","guarantees":["esm/replay-deterministic"],"kernel_version":{"major":2,"minor":0},"migration_from":{"major":1,"minor":0},"profile":{"id":"stub/esm/v1","targets_kernel":{"major":2,"minor":0},"version":{"major":1,"minor":0}},"schema_version":{"major":1,"minor":0},"seed":{"fixed":7},"source":"stub"}},"world_truth":[]}' > "$epi"
printf 'wrote %s\n' "$epi"
if [ -n "$mind" ]; then
  printf '%s\n' '{"contract":"enaction-mind-trace/1","producer":{"abi":"1.1","kernel":"stub"},"profile":{"id":"stub","version":"0.0.0"},"ticks":[{"affect":{"attention":7500,"caused_by":[17],"token":"angry"},"agent":"agent-a","conation":{"incumbent":"engage","ranking":["evade","engage"],"selected":"engage","target":"contact-1"},"tells":["fixate"],"tick":41}],"vocabulary":{"affect_tokens":["calm","angry"],"candidates":["evade","engage"],"tells":["fixate"]}}' > "$mind"
  printf 'wrote %s\n' "$mind"
fi
