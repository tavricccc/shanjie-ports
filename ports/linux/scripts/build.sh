#!/usr/bin/env bash
# Builds the Linux package folder: the engine, the upstream data (ports/data/assemble.sh), the overlay defaults merged
# for Linux (ports/overlay/merge.sh), the icon and the install scripts. Nothing here is shared with the Windows build.
#   ports/linux/scripts/build.sh DEST        DEST/install.sh then installs it
# Needs the data assets fetched once: ports/data/fetch.sh.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
linux="$(cd "$here/.." && pwd)"
root="$(cd "$linux/../.." && pwd)"
[ $# -eq 1 ] || { echo "usage: ports/linux/scripts/build.sh DEST" >&2; exit 1; }
dest="$1"
version="$("$root/ports/version.sh")"

cargo build --release --locked --manifest-path "$linux/Cargo.toml"

rm -rf "$dest"
mkdir -p "$dest/files/libexec" "$dest/files/share/shanjie/icons" "$dest/templates"
install -m755 "$linux/target/release/ibus-engine-shanjie" "$dest/files/libexec/"
"$root/ports/data/assemble.sh" "$dest/files/share/shanjie/data"
"$root/ports/overlay/merge.sh" linux "$dest/files/share/shanjie/data/defaults.ini"
install -m644 "$root/packaging/AppIcon.icon/Assets/glyph.svg" "$dest/files/share/shanjie/icons/shanjie.svg"
install -m644 "$linux/package/shanjie.xml.in" "$dest/templates/"
install -m755 "$linux/package/install.sh" "$linux/package/uninstall.sh" "$dest/"
install -m644 "$linux/package/README.txt" "$dest/"
echo "$version" > "$dest/VERSION"
echo "$dest ($version)"
