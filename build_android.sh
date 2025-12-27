#!/bin/bash
#
# Quest Shadowplay - Android Build Script
#
# This script builds the Rust library for Android (Quest 3)
#
# Prerequisites:
#   - Rust with aarch64-linux-android target
#   - Android NDK (set ANDROID_NDK_HOME)
#   - cargo-ndk installed
#
# Usage:
#   ./build_android.sh [debug|release]

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Build type (default: release)
BUILD_TYPE="${1:-release}"

echo "╔════════════════════════════════════════════════════════════╗"
echo "║           Quest Shadowplay - Android Build                 ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

# Check Rust
if ! command -v rustc &> /dev/null; then
    echo -e "${RED}Error: Rust not found. Install from https://rustup.rs${NC}"
    exit 1
fi
echo "  ✓ Rust $(rustc --version | cut -d' ' -f2)"

# Check cargo-ndk
if ! command -v cargo-ndk &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-ndk...${NC}"
    cargo install cargo-ndk
fi
echo "  ✓ cargo-ndk installed"

# Check Android target
if ! rustup target list --installed | grep -q "aarch64-linux-android"; then
    echo -e "${YELLOW}Adding Android target...${NC}"
    rustup target add aarch64-linux-android
fi
echo "  ✓ Android target (aarch64-linux-android)"

# Check NDK
if [ -z "$ANDROID_NDK_HOME" ]; then
    # Try to find NDK in common locations
    POSSIBLE_NDK_PATHS=(
        "$HOME/Android/Sdk/ndk"
        "$HOME/Library/Android/sdk/ndk"
        "/usr/local/lib/android/sdk/ndk"
        "$ANDROID_HOME/ndk"
    )
    
    for path in "${POSSIBLE_NDK_PATHS[@]}"; do
        if [ -d "$path" ]; then
            # Find the latest NDK version
            NDK_VERSION=$(ls "$path" 2>/dev/null | sort -V | tail -n1)
            if [ -n "$NDK_VERSION" ]; then
                export ANDROID_NDK_HOME="$path/$NDK_VERSION"
                break
            fi
        fi
    done
fi

if [ -z "$ANDROID_NDK_HOME" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
    echo -e "${RED}Error: Android NDK not found.${NC}"
    echo "Please set ANDROID_NDK_HOME environment variable."
    echo "Download NDK from: https://developer.android.com/ndk/downloads"
    exit 1
fi
echo "  ✓ Android NDK: $ANDROID_NDK_HOME"

echo ""
echo -e "${YELLOW}Building for Android (Quest 3)...${NC}"
echo "  Target: aarch64-linux-android (ARM64)"
echo "  Build type: $BUILD_TYPE"
echo ""

# Build
if [ "$BUILD_TYPE" == "release" ]; then
    cargo ndk -t arm64-v8a build --release
    OUTPUT_DIR="target/aarch64-linux-android/release"
else
    cargo ndk -t arm64-v8a build
    OUTPUT_DIR="target/aarch64-linux-android/debug"
fi

# Check output
if [ -f "$OUTPUT_DIR/libquest_shadowplay.so" ]; then
    echo ""
    echo -e "${GREEN}Build successful!${NC}"
    echo ""
    echo "Output: $OUTPUT_DIR/libquest_shadowplay.so"
    echo "Size: $(du -h "$OUTPUT_DIR/libquest_shadowplay.so" | cut -f1)"
    echo ""
    echo "Next steps:"
    echo "  1. Create APK using Android Studio or gradle"
    echo "  2. Install on Quest: adb install -r your-app.apk"
else
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi

