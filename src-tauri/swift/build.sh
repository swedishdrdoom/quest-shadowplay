#!/bin/bash
# Build Swift capture library for Rust FFI

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Output directory
OUT_DIR="${1:-../target/swift}"
mkdir -p "$OUT_DIR"

echo "Building Swift capture library..."

# Compile Swift to object file
swiftc \
    -O \
    -whole-module-optimization \
    -emit-library \
    -emit-module \
    -module-name CaptureKit \
    -o "$OUT_DIR/libCaptureKit.dylib" \
    CaptureController.swift \
    -Xlinker -install_name -Xlinker @rpath/libCaptureKit.dylib

# Also create a static library
swiftc \
    -O \
    -whole-module-optimization \
    -emit-object \
    -module-name CaptureKit \
    -o "$OUT_DIR/CaptureKit.o" \
    CaptureController.swift

ar rcs "$OUT_DIR/libCaptureKit.a" "$OUT_DIR/CaptureKit.o"

echo "Built:"
ls -la "$OUT_DIR"/libCaptureKit.*

echo "Done!"


