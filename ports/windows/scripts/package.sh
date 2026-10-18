#!/usr/bin/env bash
# Builds the release archive build/ports/dist/shanjie-windows-<version>-x64.zip and its .sha256. UNVERIFIED ON WINDOWS.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
version="$("$root/ports/version.sh")"
name="shanjie-windows-$version-x64"
dist="$root/build/ports/dist"
mkdir -p "$dist"
"$here/build.sh" "$dist/$name"
rm -f "$dist/$name.zip"
(cd "$dist" && zip -qr "$name.zip" "$name")
(cd "$dist" && sha256sum "$name.zip" > "$name.zip.sha256")
echo "$dist/$name.zip"
