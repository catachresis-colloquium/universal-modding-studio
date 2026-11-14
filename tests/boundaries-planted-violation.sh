#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
#
# Self-test for scripts/check-architecture-boundaries.sh.
#
# Builds a minimal fake estate (UMS, Enaction, IDApTIK, Slavia) in a fresh
# temporary directory, copies the gate into the fake UMS tree, and requires:
#   * a positive control: the gate passes on the clean estate (otherwise
#     every failure below would prove nothing);
#   * each planted violation makes the gate exit non-zero AND print the
#     expected diagnostic, so a failure for an unrelated reason does not count.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
gate_source="$repo_root/scripts/check-architecture-boundaries.sh"
test -f "$gate_source" || { echo "FAIL: gate missing: $gate_source" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

failures=0
pass() { echo "ok: $1"; }
fail() { echo "FAIL: $1" >&2; failures=$((failures + 1)); }

write() { mkdir -p "$(dirname "$1")"; printf '%s\n' "$2" >"$1"; }

# A clean estate under "$1": every sibling is a (fake) git checkout.
make_estate() {
  local estate=$1
  local ums=$estate/universal-modding-studio
  mkdir -p "$ums/.git" "$ums/scripts"
  cp "$gate_source" "$ums/scripts/check-architecture-boundaries.sh"
  write "$ums/Cargo.toml" '[workspace]
members = ["crates/ums-core"]'
  write "$ums/crates/ums-core/Cargo.toml" '[package]
name = "ums-core"
[dependencies]
serde = "1"'
  write "$ums/crates/ums-profile-sdk/src/main.rs" '    CompileIdaptik {
        #[arg(long)]
        idaptik_root: PathBuf,
    },'
  write "$ums/profiles/idaptik/v1/ghost-lobby.ums.json" '{}'
  write "$ums/profiles/slavia/v1/profile.json" '{}'

  local enaction=$estate/enaction-engine
  mkdir -p "$enaction/.git"
  write "$enaction/Cargo.toml" '[workspace]
members = ["crates/enaction-core"]'
  write "$enaction/crates/enaction-core/Cargo.toml" '[package]
name = "enaction-core"'

  local idaptik=$estate/IDApTIK
  mkdir -p "$idaptik/.git"
  write "$idaptik/Cargo.toml" '[workspace]
members = ["crates/idaptik-core"]'
  write "$idaptik/crates/idaptik-core/Cargo.toml" '[package]
name = "idaptik-core"'
  write "$idaptik/crates/idaptik-core/src/lib.rs" 'use std::fmt;'
  write "$idaptik/contracts/idaptik/v1/contract.json" '{}'
  write "$idaptik/contracts/idaptik/v1/package.schema.json" '{}'

  local slavia=$estate/chronicles-of-slavia
  mkdir -p "$slavia/.git"
  write "$slavia/engine/Cargo.toml" '[workspace]
members = ["slavia-core"]'
  write "$slavia/engine/slavia-core/Cargo.toml" '[package]
name = "slavia-core"'
  write "$slavia/engine/slavia-core/src/lib.rs" 'use std::fmt;'
}

# Run the copied gate inside an estate with a clean environment so the
# caller's *_ROOT variables cannot leak in. Extra NAME=VALUE pairs follow.
run_gate() {
  local estate=$1
  shift
  env -i PATH="$PATH" HOME="$HOME" "$@" \
    bash "$estate/universal-modding-studio/scripts/check-architecture-boundaries.sh"
}

# expect_fail NAME EXPECTED-DIAGNOSTIC SETUP-FUNCTION [NAME=VALUE...]
expect_fail() {
  local name=$1 expected=$2 setup=$3
  shift 3
  local estate=$work/$name
  make_estate "$estate"
  "$setup" "$estate"
  local out status=0
  out=$(run_gate "$estate" "$@" 2>&1) || status=$?
  if [[ $status -eq 0 ]]; then
    fail "$name: gate passed on a planted violation"
  elif ! grep -qF -- "$expected" <<<"$out"; then
    fail "$name: gate failed (exit $status) without the expected diagnostic '$expected'; got: $out"
  else
    pass "$name (exit $status: $expected)"
  fi
}

# --- positive control -------------------------------------------------------
make_estate "$work/clean"
if out=$(run_gate "$work/clean" 2>&1); then
  pass "clean estate passes"
else
  fail "clean estate did not pass; every planted case below would be vacuous: $out"
fi

# Explicit *_ROOT variables pointing elsewhere are honoured.
make_estate "$work/moved"
mkdir -p "$work/elsewhere"
mv "$work/moved/enaction-engine" "$work/elsewhere/enaction-x"
mv "$work/moved/IDApTIK" "$work/elsewhere/idaptik-x"
mv "$work/moved/chronicles-of-slavia" "$work/elsewhere/slavia-x"
if out=$(run_gate "$work/moved" \
  ENACTION_ROOT="$work/elsewhere/enaction-x" \
  IDAPTIK_ROOT="$work/elsewhere/idaptik-x" \
  SLAVIA_ROOT="$work/elsewhere/slavia-x" 2>&1); then
  pass "ENACTION_ROOT, IDAPTIK_ROOT and SLAVIA_ROOT are honoured"
else
  fail "explicit *_ROOT variables were not honoured: $out"
fi

# --- planted violations -----------------------------------------------------
plant_enaction_dep() {
  printf '[dependencies]\nums-profile-sdk = { path = "../ums" }\n' >>"$1/enaction-engine/crates/enaction-core/Cargo.toml"
}
plant_idaptik_dep() {
  printf '[dependencies]\nuniversal-modding-studio = "0.1"\n' >>"$1/IDApTIK/crates/idaptik-core/Cargo.toml"
}
plant_idaptik_import() {
  printf 'use slavia_core::Zone;\n' >>"$1/IDApTIK/crates/idaptik-core/src/lib.rs"
}
plant_slavia_dep() {
  printf '[dependencies]\nidaptik-core = "0.1"\n' >>"$1/chronicles-of-slavia/engine/slavia-core/Cargo.toml"
}
plant_slavia_import() {
  printf 'use idaptik_core::Level;\n' >>"$1/chronicles-of-slavia/engine/slavia-core/src/lib.rs"
}
plant_ums_links_game() {
  printf 'idaptik-core = { path = "../../../IDApTIK/crates/idaptik-core" }\n' >>"$1/universal-modding-studio/crates/ums-core/Cargo.toml"
}
plant_compiler_loses_path_root() {
  printf 'fn main() {}\n' >"$1/universal-modding-studio/crates/ums-profile-sdk/src/main.rs"
}
plant_unreadable_manifests() {
  # An Enaction tree whose crate manifests cannot be found must not be read
  # as "no forbidden dependency".
  rm -rf "$1/enaction-engine/crates"
}
plant_only_affective_engine() {
  # The pre-rename name must not be accepted as a fallback.
  mv "$1/enaction-engine" "$1/affective-engine"
}
plant_missing_contract() {
  rm "$1/IDApTIK/contracts/idaptik/v1/contract.json"
}
no_plant() { :; }

expect_fail enaction-depends-on-ums "Enaction must not depend on UMS" plant_enaction_dep
expect_fail idaptik-depends-on-ums "IDApTIK runtime must not depend on UMS or Slavia" plant_idaptik_dep
expect_fail idaptik-imports-slavia "IDApTIK source must not import Slavia profile types" plant_idaptik_import
expect_fail slavia-depends-on-idaptik "Slavia runtime must not depend on UMS or IDApTIK" plant_slavia_dep
expect_fail slavia-imports-idaptik "Slavia source must not import IDApTIK profile types" plant_slavia_import
expect_fail ums-links-game "UMS Rust core must not link either game runtime" plant_ums_links_game
expect_fail compiler-loses-path-root "compiler no longer takes a game root by path" plant_compiler_loses_path_root
expect_fail unreadable-manifests "cannot scan" plant_unreadable_manifests
expect_fail no-affective-engine-fallback "missing sibling repository: " plant_only_affective_engine
expect_fail missing-contract "missing required file" plant_missing_contract
expect_fail missing-sibling "missing sibling repository: " no_plant ENACTION_ROOT="$work/does-not-exist"

# --- static: no fixed temporary path ----------------------------------------
if grep -nE '/tmp/' "$gate_source" >/dev/null; then
  fail "gate still writes a fixed /tmp path: $(grep -nE '/tmp/' "$gate_source")"
else
  pass "gate uses no fixed /tmp path"
fi

if [[ $failures -ne 0 ]]; then
  echo "boundaries-planted-violation: $failures check(s) failed" >&2
  exit 1
fi
echo "boundaries-planted-violation: all planted violations fail the gate; the clean estate passes"
