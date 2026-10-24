#!/usr/bin/env bash
# Builds a release .deb of the pagecrop app.
#
# 1. `cargo sweep` removes build artifacts older than 7 days from target/, so the folder does not
#    grow forever; anything built in the last 7 days stays and keeps the build fast.
# 2. `cargo deb` builds the app in release mode and packs it, with its desktop entry and icon, into
#    target/debian/pagecrop_<version>-1_<arch>.deb.
#
# Needs: cargo install cargo-sweep cargo-deb --locked
# Install the result with: sudo apt install ./target/debian/pagecrop_<version>-1_<arch>.deb
set -euo pipefail

cd "$(dirname "$0")/.."

for tool in sweep deb; do
    if ! cargo "$tool" --version >/dev/null 2>&1; then
        echo "error: cargo-$tool is not installed. Run: cargo install cargo-$tool --locked" >&2
        exit 1
    fi
done

echo "==> Removing build artifacts older than 7 days"
cargo sweep --time 7

echo "==> Building the release .deb"
cargo deb -p pagecrop

deb=$(ls -t target/debian/*.deb | head -n 1)
echo "==> Done: $deb"
