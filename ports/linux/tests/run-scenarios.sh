#!/usr/bin/env bash
# Runs every scenario in ports/linux/tests/scenarios against ENGINE (default: the release build) and the data in
# SHANJIE_DATA (default: build/ports/data, from ports/data/assemble.sh). Exits non-zero when any expected commit list
# differs. Adapted from the earlier Linux port.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
engine="${1:-$here/../target/release/ibus-engine-shanjie}"
export SHANJIE_DATA="${SHANJIE_DATA:-$root/build/ports/data}"
logs="$root/build/ports/linux-scenarios"
mkdir -p "$logs"
failed=0
for scenario in "$here"/scenarios/*.json; do
  name="$(basename "$scenario" .json)"
  if timeout 120 "$here/ibus-harness.sh" "$engine" "$scenario" >"$logs/$name.out" 2>"$logs/$name.err"; then
    echo "ok   $name"
  else
    echo "FAIL $name ($(grep -m1 FAIL "$logs/$name.err" || echo "see $logs/$name.err"))"
    failed=1
  fi
done
exit $failed
