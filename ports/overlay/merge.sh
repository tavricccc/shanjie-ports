#!/usr/bin/env bash
# Writes the defaults.ini a package ships: ports/overlay/defaults.ini, then ports/overlay/PLATFORM.ini, one line per key
# with the platform's value winning, in first-seen order. Comments and blank lines are dropped; a line that is not
# key=value, or whose key is not one of defaults.ini's keys, fails the build, so a typo never ships silently (the values
# themselves are checked by the shell when it loads the file: ports/shell/src/prefs.rs `Defaults::load`).
#   ports/overlay/merge.sh linux|windows OUT
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
[ $# -eq 2 ] || { echo "usage: ports/overlay/merge.sh linux|windows OUT" >&2; exit 1; }
platform="$1"; out="$2"
[ -f "$here/$platform.ini" ] || { echo "error: no overlay for $platform" >&2; exit 1; }
known="layout demoteSensitive prediction abbreviation english englishInline cjkSpacing acgPack glassTint candidateVertical rerank rerankLevel profileMode"
# The Windows habits (ports/shell/src/winops.rs; ports/docs/windows-habits.md).
known="$known win.shiftToggle win.shiftSide win.ctrlSpaceToggle win.shiftSpaceWidth win.fullWidth win.capsLockEnglish win.ctrlPunctuation win.symbolTable win.codeInput win.enterCommits win.learning win.leftContext win.mixedInput win.mixedTolerance win.punct.shift.comma win.punct.shift.period win.punct.shift.slash win.punct.shift.1 win.punct.shift.semicolon win.punct.shift.lbracket win.punct.shift.rbracket win.punct.shift.9 win.punct.shift.0 win.punct.shift.backquote win.punct.shift.backslash win.punct.shift.quote win.punct.shift.equal win.punct.plain.lbracket win.punct.plain.rbracket win.punct.plain.backslash win.punct.plain.quote win.punct.plain.equal win.punct.plain.backquote"
awk -v platform="$platform" -v known="$known" '
  BEGIN { split(known, ks, " "); for (j in ks) ok[ks[j]] = 1 }
  { sub(/\r$/, "") }
  /^[[:space:]]*(#|$)/ { next }
  {
    i = index($0, "=")
    if (i == 0) { printf "error: %s:%d is not key=value\n", FILENAME, FNR > "/dev/stderr"; bad = 1; next }
    k = substr($0, 1, i - 1); v = substr($0, i + 1)
    gsub(/^[[:space:]]+|[[:space:]]+$/, "", k); gsub(/^[[:space:]]+|[[:space:]]+$/, "", v)
    if (!(k in ok)) { printf "error: %s:%d: unknown key %s\n", FILENAME, FNR, k > "/dev/stderr"; bad = 1; next }
    if (!(k in val)) order[++n] = k
    val[k] = v
  }
  END {
    if (bad) exit 1
    print "# Overlay defaults (ports/overlay: defaults.ini + " platform ".ini). Empty: upstream behavior."
    for (j = 1; j <= n; j++) print order[j] "=" val[order[j]]
  }
' "$here/defaults.ini" "$here/$platform.ini" > "$out.tmp"
mv "$out.tmp" "$out"
