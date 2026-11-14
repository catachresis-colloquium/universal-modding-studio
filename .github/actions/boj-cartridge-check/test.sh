#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT

GITHUB_WORKSPACE=$fixture BOJ_CARTRIDGE_REQUIRED=false "$here/check.sh"

if GITHUB_WORKSPACE=$fixture BOJ_CARTRIDGE_REQUIRED=true "$here/check.sh"; then
  echo 'required-but-absent BoJ cartridge unexpectedly passed' >&2
  exit 1
fi

printf '%s\n' '{"name":"controlled-fixture"}' > "$fixture/cartridge.json"
GITHUB_WORKSPACE=$fixture BOJ_CARTRIDGE_REQUIRED=true "$here/check.sh"

echo 'boj-cartridge-check controls passed'
