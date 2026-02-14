#!/usr/bin/env bash
set -e

RUSTPEEK="./target/debug/rustpeek.exe"
TEMP_DIR="samples_fixtest"

rm -rf "$TEMP_DIR"
cp -r samples "$TEMP_DIR"

echo "rustpeek fix test - check, fix, check"
echo

for d in "$TEMP_DIR"/*/; do
    name=$(basename "$d")
    echo "=== $name ==="
    echo "-- check --"
    $RUSTPEEK check "$d" 2>&1 || true
    echo "-- fix --"
    $RUSTPEEK fix "$d" 2>&1 || true
    echo "-- recheck --"
    $RUSTPEEK check "$d" 2>&1 || true
    echo
done

rm -rf "$TEMP_DIR"
