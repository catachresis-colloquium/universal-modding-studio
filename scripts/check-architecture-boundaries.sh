#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Executable form of ADR-0016: UMS is never a runtime dependency, and the two
# game profiles never import each other. A missing sibling, an unreadable
# manifest or a missing contract file is a failure, never a skip.
#
# Sibling roots default to siblings of this checkout and can be overridden
# with ENACTION_ROOT, IDAPTIK_ROOT and SLAVIA_ROOT. There is no fallback to
# any other directory name. Self-test: tests/boundaries-planted-violation.sh.
set -euo pipefail

ums_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
meta_root=$(cd "$ums_root/.." && pwd)
enaction_root=${ENACTION_ROOT:-"$meta_root/enaction-engine"}
idaptik_root=${IDAPTIK_ROOT:-"$meta_root/IDApTIK"}
slavia_root=${SLAVIA_ROOT:-"$meta_root/chronicles-of-slavia"}

for root in "$ums_root" "$idaptik_root" "$slavia_root" "$enaction_root"; do
  test -d "$root/.git" -o -f "$root/.git" || {
    echo "missing sibling repository: $root" >&2
    exit 1
  }
done

match_file=$(mktemp)
trap 'rm -f "$match_file"' EXIT

# rg exits 0 on a match, 1 on no match and 2 on an error (for example an
# unexpanded glob naming no file). Only exit 1 is a pass.
fail_match() {
  local label=$1
  local pattern=$2
  shift 2
  local status=0
  rg -n -i "$pattern" "$@" >"$match_file" 2>&1 || status=$?
  case $status in
    0)
      echo "forbidden dependency: $label" >&2
      sed -n '1,40p' "$match_file" >&2
      return 1
      ;;
    1) return 0 ;;
    *)
      echo "cannot scan for: $label (rg exit $status)" >&2
      sed -n '1,40p' "$match_file" >&2
      return 1
      ;;
  esac
}

fail_match "Enaction must not depend on UMS" \
  'ums-|universal.modding|idaptik-ums' \
  "$enaction_root/Cargo.toml" "$enaction_root/crates"/*/Cargo.toml
fail_match "IDApTIK runtime must not depend on UMS or Slavia" \
  'ums-|universal.modding|slavia[_-](core|profile)' \
  "$idaptik_root/Cargo.toml" "$idaptik_root/crates"/*/Cargo.toml
fail_match "IDApTIK source must not import Slavia profile types" \
  '(^|[[:space:]])(use|extern[[:space:]]+crate)[[:space:]].*slavia|slavia_profile' \
  "$idaptik_root/crates"
fail_match "Slavia runtime must not depend on UMS or IDApTIK" \
  'ums-|universal.modding|idaptik[_-](core|profile)' \
  "$slavia_root/engine/Cargo.toml" "$slavia_root/engine"/*/Cargo.toml
fail_match "Slavia source must not import IDApTIK profile types" \
  '(^|[[:space:]])(use|extern[[:space:]]+crate)[[:space:]].*idaptik|idaptik_profile' \
  "$slavia_root/engine"
fail_match "UMS Rust core must not link either game runtime" \
  '^[[:space:]]*(idaptik-core|slavia-core|chronicles-of-slavia)[[:space:]]*=' \
  "$ums_root/Cargo.toml" "$ums_root/crates"/*/Cargo.toml

# Positive assertion (ADR-0016): the IDApTIK compiler takes the game root as
# a path argument. Converting it into a crate dependency is the regression a
# forbidden-pattern list would not catch.
if ! rg -q '^[[:space:]]*idaptik_root:[[:space:]]*PathBuf' "$ums_root/crates/ums-profile-sdk/src/main.rs"; then
  echo "compiler no longer takes a game root by path: expected 'idaptik_root: PathBuf' (--idaptik-root) in crates/ums-profile-sdk/src/main.rs" >&2
  exit 1
fi

for required in \
  "$idaptik_root/contracts/idaptik/v1/contract.json" \
  "$idaptik_root/contracts/idaptik/v1/package.schema.json" \
  "$ums_root/profiles/idaptik/v1/ghost-lobby.ums.json" \
  "$ums_root/profiles/slavia/v1/profile.json"; do
  test -f "$required" || {
    echo "missing required file: $required" >&2
    exit 1
  }
done

echo "architecture boundaries: dependency direction and profile isolation verified"
