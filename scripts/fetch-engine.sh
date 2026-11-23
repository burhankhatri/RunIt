#!/bin/sh
# Fetch the pinned llama.cpp build for a platform into engines/, keeping only what RunIt runs.
set -eu
cd "$(dirname "$0")/.."
TAG=b12651
case "$1" in
  macos-arm64) FILE=llama-$TAG-bin-macos-arm64.tar.gz; SUM=264fd872de61679c45ad57e1e21c2a2a54e29b35734ecf3d50f24166447ab8d2 ;;
  macos-x86_64) FILE=llama-$TAG-bin-macos-x64.tar.gz; SUM=34c19771545479a783d3b68c446383ab703033890c052d3ff0b860a5c9f59e56 ;;
  *) echo "usage: fetch-engine.sh macos-arm64|macos-x86_64" >&2; exit 1 ;;
esac
OUT=engines/$1
[ "$(cat "$OUT/.tag" 2>/dev/null)" = "$TAG" ] && exit 0
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
curl -fsSL --retry 3 -o "$TMP/e.tgz" "https://github.com/ggml-org/llama.cpp/releases/download/$TAG/$FILE"
echo "$SUM  $TMP/e.tgz" | shasum -a 256 -c - >/dev/null
tar xzf "$TMP/e.tgz" -C "$TMP"
rm -rf "$OUT"; mkdir -p "$OUT"
SRC=$(dirname "$(find "$TMP" -name llama-bench -type f)")
for t in llama-bench llama-server; do cp "$SRC/$t" "$OUT/"; done
for l in ggml ggml-base ggml-cpu ggml-metal ggml-blas llama llama-common llama-bench-impl llama-server-impl mtmd; do
  cp -R "$SRC"/lib$l.*dylib "$OUT/" 2>/dev/null || true
done
# RPC backend is linked by the loader on some builds.
cp -R "$SRC"/libggml-rpc.*dylib "$OUT/" 2>/dev/null || true
cp "$SRC/LICENSE" "$OUT/LICENSE-llama.cpp"
echo $TAG > "$OUT/.tag"
