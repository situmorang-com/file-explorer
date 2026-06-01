#!/usr/bin/env bash
# Capture the running File Explorer window into docs/screenshot.png.
# Then uncomment the <img> line in README.md.
#
# Usage:
#   1. Start the app (cargo run --release -- ~/code)
#   2. In another terminal: ./scripts/screenshot.sh
#   3. Click the File Explorer window when the crosshair appears.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$ROOT/docs"
OUT="$ROOT/docs/screenshot.png"

echo "Click the File Explorer window…"
# -o omits the drop shadow; -w captures a single window after click
screencapture -o -w "$OUT"
echo "wrote $OUT"
echo "Next: uncomment ![file-explorer](docs/screenshot.png) in README.md and git add docs/screenshot.png"
