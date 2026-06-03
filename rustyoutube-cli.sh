#!/usr/bin/env bash
set -euo pipefail

BINARY="./target/release/rustyoutube-cli"

# Build dulu (hanya jika ada perubahan source code)
if [ ! -f "$BINARY" ] || [ "$(find src/ -newer "$BINARY" -type f -name '*.rs' 2>/dev/null | head -1)" != "" ]; then
    echo "==> Compilasi ulang (ada perubahan source)..."
    cargo build --release
else
    echo "==> Binary sudah up-to-date, skip compile."
fi

# Jalankan binary langsung tanpa compile
exec "$BINARY" "$@"
