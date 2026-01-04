#!/bin/bash
# Regenerate Cargo.lock without local patches for committing
#
# Usage: ./scripts/clean-lockfile.sh

set -e
cd "$(dirname "$0")/.."

if [[ ! -f .cargo/config.toml ]]; then
    echo "No .cargo/config.toml found, lockfile should already be clean"
    exit 0
fi

echo "Temporarily disabling local patches..."
mv .cargo/config.toml .cargo/config.toml.bak

echo "Regenerating lockfile from crates.io..."
rm Cargo.lock
cargo generate-lockfile

echo "Restoring local patches..."
mv .cargo/config.toml.bak .cargo/config.toml

echo "Done! Cargo.lock now uses crates.io versions."
echo "You can safely commit it."
