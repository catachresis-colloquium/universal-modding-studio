#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$here/../../.." && pwd)
readme=${1:-"$repo_root/README.adoc"}

fail() {
  echo "README badge regression: $*" >&2
  exit 1
}

assert_absent() {
  local needle=$1
  local description=$2

  if grep -Fqi -- "$needle" "$readme"; then
    fail "$description"
  fi
}

assert_exactly_once() {
  local needle=$1
  local description=$2
  local count

  count=$(grep -Fxc -- "$needle" "$readme" || true)
  if [ "$count" -ne 1 ]; then
    fail "$description (found $count occurrences)"
  fi
}

[ -f "$readme" ] || fail "README not found at $readme"

# Project 8509 belongs to another repository. Guard both the specific false
# claim and broader attempts to restore an unverified Best Practices badge.
assert_absent \
  'bestpractices.dev/projects/8509' \
  'unrelated OpenSSF Best Practices project 8509 must remain absent'
assert_absent \
  'bestpractices.dev/projects/' \
  'an OpenSSF Best Practices project must not be claimed without registration'
assert_absent \
  'OpenSSF Best Practices' \
  'the unearned OpenSSF Best Practices label must remain absent'

# Removing the false badge must not remove or duplicate the repository-owned
# Scorecard badge that provides the remaining OpenSSF status signal.
scorecard_badge='image:https://api.scorecard.dev/projects/github.com/metadatastician/universal-modding-studio/badge[OpenSSF Scorecard,link="https://scorecard.dev/viewer/?uri=github.com/metadatastician/universal-modding-studio"]'
assert_exactly_once \
  "$scorecard_badge" \
  'the repository-specific OpenSSF Scorecard badge must appear exactly once'

# The removed badge occupied the final line of the compliance block. Preserve
# the blank separator so the following quote cannot be parsed as badge content.
scorecard_line=$(grep -nFx -- "$scorecard_badge" "$readme" | cut -d: -f1)
separator=$(sed -n "$((scorecard_line + 1))p" "$readme")
following_line=$(sed -n "$((scorecard_line + 2))p" "$readme")
[ -z "$separator" ] || fail 'the compliance block must end with a blank line'
[ "$following_line" = '[quote]' ] || fail 'the quote block must follow the compliance separator'

echo 'README badge regression checks passed'
