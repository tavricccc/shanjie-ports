#!/usr/bin/env bash
# Assembles the runtime data folder of a port: exactly the files upstream's scripts/build-app.sh copies into the macOS
# app's Contents/Resources (lexicon, digits, the ACG pack, the five model files, english-words.txt and the licenses),
# under the same names and layout, so the core opens it as it opens the app's Resources. The two attribution files
# build-app.sh writes inline are taken from build-app.sh itself, so they cannot drift from it.
#
#   ports/data/assemble.sh DEST       (run ports/data/fetch.sh first)
#
# Nothing platform-specific happens here; each platform's package script copies DEST into its own package.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
fail() { echo "error: $*" >&2; exit 1; }
[ $# -eq 1 ] || fail "usage: ports/data/assemble.sh DEST"
DEST="$1"

for f in data/lm/bigram.sjlm data/lm/classes.sjc data/lm/kn.sjkn data/lm/tinylm.sjtl data/lm/trigram.sjt3; do
  [ -f "$f" ] || fail "$f is missing; run ports/data/fetch.sh"
done
for f in bigram.sjlm classes.sjc kn.sjkn tinylm.sjtl trigram.sjt3; do
  sha256sum -c --quiet "data/$f.sha256" || fail "data/lm/$f does not match data/$f.sha256; run ports/data/fetch.sh"
done

EN_IN="${SHANJIE_ENGLISH_CACHE:-$HOME/.cache/shanjie/english}/frequency_dictionary_en_82_765.txt"
[ -f "$EN_IN" ] || fail "$EN_IN is missing; run ports/data/fetch.sh"
EN_COMMIT="$(python3 tools/build_english_words.py --print-commit)"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
RES="$STAGE/data"
mkdir -p "$RES/packs" "$RES/LICENSES"

# The file list of build-app.sh, in its order.
cp data/lexicon/mcbpmf-data.txt data/lexicon/overlay-add.tsv data/lexicon/sandhi-add.tsv data/lexicon/names-add.tsv data/lexicon/demote.tsv "$RES/"
cp data/lexicon/digits.tsv "$RES/"
cp data/packs/acg-add.tsv data/packs/acg-sources.tsv data/packs/acg.json "$RES/packs/"
cp -L data/lm/bigram.sjlm data/lm/classes.sjc data/lm/kn.sjkn data/lm/tinylm.sjtl data/lm/trigram.sjt3 "$RES/"
python3 tools/build_english_words.py --input "$EN_IN" --out "$RES/english-words.txt" >/dev/null || fail "tools/build_english_words.py failed"
[ "$(sha256sum "$RES/english-words.txt" | awk '{ print $1 }')" = "$(awk 'NR == 1 { print $1 }' data/english-words.txt.sha256)" ] \
  || fail "the generated english-words.txt does not match data/english-words.txt.sha256"

cp LICENSE "$RES/LICENSES/LICENSE"
cp LICENSES/McBopomofo-MIT.txt LICENSES/data.md LICENSES/SCOWL.txt LICENSES/SymSpell-MIT.txt LICENSES/name-suggestion-index-BSD-3-Clause.txt "$RES/LICENSES/"

# The heredoc bodies of build-app.sh: the first one verbatim ('EOF', no expansion), the second with $EN_COMMIT expanded
# (an unquoted heredoc whose only expansion is that variable).
heredoc() {  # heredoc <file name written by build-app.sh>
  awk -v target="$1" '
    index($0, target) && /<</ { on = 1; next }
    on && $0 == "EOF" { exit }
    on { print }
  ' scripts/build-app.sh
}
heredoc 'CC-BY-SA-4.0-attribution.txt' > "$RES/LICENSES/CC-BY-SA-4.0-attribution.txt"
[ -s "$RES/LICENSES/CC-BY-SA-4.0-attribution.txt" ] || fail "cannot find the CC BY-SA attribution in scripts/build-app.sh"
cat LICENSES/name-suggestion-index-BSD-3-Clause.txt >> "$RES/LICENSES/CC-BY-SA-4.0-attribution.txt"
heredoc 'english-words-attribution.txt' | sed "s/\$EN_COMMIT/$EN_COMMIT/g" > "$RES/LICENSES/english-words-attribution.txt"
[ -s "$RES/LICENSES/english-words-attribution.txt" ] || fail "cannot find the English attribution in scripts/build-app.sh"
grep -q '\$' "$RES/LICENSES/english-words-attribution.txt" && fail "an unexpanded variable is left in the English attribution"

# What this folder holds, with the upstream commit it was assembled from.
{
  echo "upstream_commit=$(git rev-parse HEAD)"
  (cd "$RES" && find . -type f ! -name MANIFEST | LC_ALL=C sort | xargs sha256sum)
} > "$RES/MANIFEST"

rm -rf "$DEST"
mkdir -p "$(dirname "$DEST")"
mv "$RES" "$DEST"
echo "assembled $DEST"
