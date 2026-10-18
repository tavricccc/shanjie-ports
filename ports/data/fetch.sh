#!/usr/bin/env bash
# Downloads and checks the data upstream's scripts/build-app.sh requires before it builds the macOS app:
# the five model release assets into data/lm/ (as upstream's own instructions do) and the SymSpell input of
# english-words.txt into the cache. Every file is checked against the SHA-256 upstream records in data/*.sha256
# or tools/build_english_words.py; a mismatch fails. Nothing here chooses a different file than the macOS app ships.
#
#   ports/data/fetch.sh
#
# Environment: SHANJIE_ENGLISH_CACHE (default ~/.cache/shanjie/english, the same default as build-app.sh).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
fail() { echo "error: $*" >&2; exit 1; }
sha() { sha256sum "$1" | awk '{ print $1 }'; }

# release tag, asset, recorded checksum file: the order and the pairs of build-app.sh.
ASSETS=(
  "model-v6 bigram.sjlm data/bigram.sjlm.sha256"
  "classes-v5 classes.sjc data/classes.sjc.sha256"
  "kn-v1 kn.sjkn data/kn.sjkn.sha256"
  "sj-sr-6-14m tinylm.sjtl data/tinylm.sjtl.sha256"
  "sj-cx-1-7.7m trigram.sjt3 data/trigram.sjt3.sha256"
)
REPO="https://github.com/Nanako0129/shanjie/releases/download"
mkdir -p data/lm
for entry in "${ASSETS[@]}"; do
  read -r tag name sumfile <<<"$entry"
  want="$(awk 'NR == 1 { print $1 }' "$sumfile")"
  out="data/lm/$name"
  if [ -f "$out" ] && [ "$(sha "$out")" = "$want" ]; then
    continue
  fi
  echo "downloading $tag/$name"
  rm -f "$out.part"
  curl -fsSL --retry 3 -o "$out.part" "$REPO/$tag/$name" || fail "cannot download $tag/$name"
  [ "$(sha "$out.part")" = "$want" ] || { rm -f "$out.part"; fail "$tag/$name does not match $sumfile"; }
  mv "$out.part" "$out"
done

EN_URL="$(python3 tools/build_english_words.py --print-url)"
EN_IN_SHA="$(python3 tools/build_english_words.py --print-input-sha256)"
EN_IN="${SHANJIE_ENGLISH_CACHE:-$HOME/.cache/shanjie/english}/frequency_dictionary_en_82_765.txt"
if ! { [ -f "$EN_IN" ] && [ "$(sha "$EN_IN")" = "$EN_IN_SHA" ]; }; then
  mkdir -p "$(dirname "$EN_IN")"
  rm -f "$EN_IN"
  curl -fsSL --retry 3 -o "$EN_IN.part" "$EN_URL" || fail "cannot download the English word list input from $EN_URL"
  [ "$(sha "$EN_IN.part")" = "$EN_IN_SHA" ] || { rm -f "$EN_IN.part"; fail "the English word list input does not match $EN_IN_SHA"; }
  mv "$EN_IN.part" "$EN_IN"
fi
echo "data ready: data/lm/ and $EN_IN"
