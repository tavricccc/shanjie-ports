#!/usr/bin/env bash
# The ports' version: the upstream release the branch is based on, the upstream commits since it, and the ports commit,
# e.g. 0.4.7+12.ports.722e751 (the ports commit with -dirty when the tree has changes).
set -euo pipefail
cd "$(dirname "$0")/.."
base="$(git merge-base HEAD upstream/main 2>/dev/null || git merge-base HEAD origin/main)"
describe="$(git describe --tags --match 'v[0-9]*' "$base")"
release="${describe#v}"; release="${release%%-*}"
since="$(git rev-list --count "v$release..$base")"
echo "$release+$since.ports.$(git rev-parse --short HEAD)$(git diff --quiet HEAD -- . 2>/dev/null || echo -dirty)"
