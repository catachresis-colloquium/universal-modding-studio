#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Mimics the command line of idaptik-weevil-replay: each document goes to its
# own file (--emit-epistemic PATH, optional --emit-mind-trace PATH), and
# stdout is a human summary that names the paths, so stdout differs between
# runs while the documents do not. --mode selects a deviation for a test:
# same (default), vary-mind, tamper, skip-mind, nothing, big.
# --log FILE appends the runner-output path to FILE.
epi=""; mind=""; mode="same"; log=""
while [ $# -gt 0 ]; do
  case "$1" in
    --emit-epistemic) [ $# -ge 2 ] || exit 2; epi="$2"; shift 2 ;;
    --emit-mind-trace) [ $# -ge 2 ] || exit 2; mind="$2"; shift 2 ;;
    --mode) mode="$2"; shift 2 ;;
    --log) log="$2"; shift 2 ;;
    --profile|--ticks|--seed) shift 2 ;;
    *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done
if [ -z "$epi" ]; then printf -- '--emit-epistemic needs a path\n' >&2; exit 2; fi
if [ -n "$log" ]; then printf '%s\n' "$epi" >> "$log"; fi
printf 'kernel      : stub\n'
if [ "$mode" = nothing ]; then exit 0; fi
proposition="door is north"
if [ "$mode" = tamper ]; then proposition="door is north TAMPERED"; fi
printf '%s\n' '{"contract":"enaction-runner-output/1","package":{"beliefs":[{"bearer":"agent-a","belief_id":1,"confidence":{"belief":8000,"plausibility":9500},"proposition":"'"$proposition"'","source_events":[17],"status":"believed","valid_from":40}],"events":[{"confidence":9000,"event_id":17,"kind":"observation","observer":"agent-a","proposition":"door is north","provenance":{"origin":"sight"},"sequence":17,"subject":{"agent":"agent-a"},"tick":40}],"manifest":{"digest":"0000000000000000000000000000000000000000000000000000000000000000","guarantees":["esm/replay-deterministic"],"kernel_version":{"major":1,"minor":0},"migration_from":{"major":1,"minor":0},"profile":{"id":"stub/esm/v1","targets_kernel":{"major":1,"minor":0},"version":{"major":1,"minor":0}},"schema_version":{"major":1,"minor":0},"seed":{"fixed":7},"source":"stub"}},"world_truth":[]}' > "$epi"
printf 'wrote %s\n' "$epi"
if [ "$mode" = big ]; then
  i=0
  while [ $i -lt 400 ]; do printf '%s\n' '                                ' >> "$epi"; i=$((i+1)); done
fi
if [ -n "$mind" ] && [ "$mode" != skip-mind ]; then
  kernel="stub"
  if [ "$mode" = vary-mind ]; then kernel="stub-$$"; fi
  printf '%s\n' '{"contract":"enaction-mind-trace/1","producer":{"abi":"1.1","kernel":"'"$kernel"'"},"profile":{"id":"stub","version":"0.0.0"},"ticks":[{"affect":{"attention":7500,"caused_by":[17],"token":"angry"},"agent":"agent-a","conation":{"incumbent":"engage","ranking":["evade","engage"],"selected":"engage","target":"contact-1"},"tells":["fixate"],"tick":41}],"vocabulary":{"affect_tokens":["calm","angry"],"candidates":["evade","engage"],"tells":["fixate"]}}' > "$mind"
  printf 'wrote %s\n' "$mind"
fi
