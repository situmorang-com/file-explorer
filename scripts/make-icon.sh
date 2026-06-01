#!/usr/bin/env bash
# Generate assets/icon.icns from a single 1024×1024 PNG using macOS-native tools.
# Usage: ./scripts/make-icon.sh path/to/icon-1024.png
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <path-to-1024x1024.png>" >&2
  exit 1
fi
SRC="$1"
[[ -f "$SRC" ]] || { echo "no such file: $SRC" >&2; exit 1; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICONSET="$ROOT/assets/icon.iconset"
OUT="$ROOT/assets/icon.icns"

rm -rf "$ICONSET"
mkdir -p "$ICONSET"

# Apple-required slot sizes.
declare -a SIZES=(16 32 64 128 256 512)
for s in "${SIZES[@]}"; do
  sips -z "$s" "$s"     "$SRC" --out "$ICONSET/icon_${s}x${s}.png"     >/dev/null
  sips -z "$((s*2))" "$((s*2))" "$SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
cp "$SRC" "$ICONSET/icon_512x512@2x.png"

iconutil -c icns -o "$OUT" "$ICONSET"
rm -rf "$ICONSET"
echo "wrote $OUT"
echo "now uncomment the icon line in Cargo.toml and run: cargo bundle --release"
