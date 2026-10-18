#!/usr/bin/env bash
# Builds the Windows package folder by cross compilation: shanjie-host.exe, shanjie_tsf.dll, the upstream data
# (ports/data/assemble.sh), the overlay defaults merged for Windows (ports/overlay/merge.sh) and the install scripts.
# Nothing here is shared with the Linux build.
#   ports/windows/scripts/build.sh DEST
# Needs: the Rust target x86_64-pc-windows-gnu and mingw-w64 (x86_64-w64-mingw32-gcc); the data assets fetched once
# (ports/data/fetch.sh). UNVERIFIED ON WINDOWS: the result has not been run on Windows.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
windows="$(cd "$here/.." && pwd)"
root="$(cd "$windows/../.." && pwd)"
[ $# -eq 1 ] || { echo "usage: ports/windows/scripts/build.sh DEST" >&2; exit 1; }
dest="$1"
version="$("$root/ports/version.sh")"
target=x86_64-pc-windows-gnu

cargo build --release --locked --target "$target" --manifest-path "$windows/Cargo.toml" -p shanjie-host -p shanjie-tsf

rm -rf "$dest"
mkdir -p "$dest/files"
install -m755 "$windows/target/$target/release/shanjie-host.exe" "$dest/files/"
install -m755 "$windows/target/$target/release/shanjie_tsf.dll" "$dest/files/"
"$root/ports/data/assemble.sh" "$dest/files/data"
"$root/ports/overlay/merge.sh" windows "$dest/files/data/defaults.ini"
install -m644 "$windows/package/install.ps1" "$windows/package/uninstall.ps1" "$dest/"
# Windows tools read README.txt best with CRLF line ends.
sed 's/$/\r/' "$windows/package/README.txt" > "$dest/README.txt"
printf '%s\r\n' "$version" > "$dest/VERSION"
echo "$dest ($version)"
