#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Mimics the command line and exit codes of enaction-validate (CAC-KERNEL
# section 9): --policies FILE is required, --mind-trace FILE is optional, the
# runner output arrives on stdin. Exit 0 echoes the envelope, then the mind
# trace, one line each. Exit 1 prints one {"faults":[...]} line: UnknownField
# at contract for input that is not a runner output, and digest-matches at
# package.manifest.digest for input marked TAMPERED (the real validator
# recomputes the digest; this stub only recognises the marker). Exit 2 is an
# invocation failure with the reason on stderr and nothing on stdout.
policies=""; mind=""
while [ $# -gt 0 ]; do
  case "$1" in
    --policies) [ $# -ge 2 ] || { printf 'enaction-validate: --policies needs a file\n' >&2; exit 2; }; policies="$2"; shift 2 ;;
    --mind-trace) [ $# -ge 2 ] || { printf 'enaction-validate: --mind-trace needs a file\n' >&2; exit 2; }; mind="$2"; shift 2 ;;
    *) printf 'enaction-validate: unknown argument %s\n' "$1" >&2; exit 2 ;;
  esac
done
if [ -z "$policies" ]; then printf 'enaction-validate: missing --policies\n' >&2; exit 2; fi
if [ ! -r "$policies" ]; then printf 'enaction-validate: cannot read %s\n' "$policies" >&2; exit 2; fi
if [ -n "$mind" ] && [ ! -r "$mind" ]; then printf 'enaction-validate: cannot read %s\n' "$mind" >&2; exit 2; fi
IFS= read -r envelope
while IFS= read -r rest; do :; done
case "$envelope" in
  *'"contract":"enaction-runner-output/1"'*) ;;
  *) printf '{"faults":[{"detail":"not an enaction-runner-output/1 envelope","path":"contract","rule":"UnknownField"}]}\n'
     printf 'UnknownField\tcontract\tnot an enaction-runner-output/1 envelope\n' >&2
     exit 1 ;;
esac
case "$envelope" in
  *TAMPERED*) printf '{"faults":[{"detail":"declared digest differs from the computed digest","path":"package.manifest.digest","rule":"digest-matches"}]}\n'
     printf 'digest-matches\tpackage.manifest.digest\tdeclared digest differs from the computed digest\n' >&2
     exit 1 ;;
esac
printf '%s\n' "$envelope"
if [ -n "$mind" ]; then
  while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done < "$mind"
fi
exit 0
