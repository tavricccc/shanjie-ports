#!/usr/bin/env bash
# Builds the release archive build/ports/dist/shanjie-linux-<version>-<arch>.tar.gz and its .sha256.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
version="$("$root/ports/version.sh")"
name="shanjie-linux-$version-$(uname -m)"
dist="$root/build/ports/dist"
mkdir -p "$dist"
"$here/build.sh" "$dist/$name"
tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
(cd "$dist" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "$dist/$name.tar.gz"
