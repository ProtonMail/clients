#!/bin/bash

# This script is meant as a way to test the Rust code from a Kotlin skeleton app

set -e

# Determine the directory where the script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Dynamically determine the cargo target directory
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps | jq -r '.target_directory')
#TARGET_DIR="$SCRIPT_DIR/target"

OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
STRIP_BIN=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/${OS_NAME}-x86_64/bin/llvm-strip

ANDROID_MODULE_DIR="../../android/core-telemetry-ffi"
LIB_NAME="libcore_telemetry.so"
OUT_DIR="$TARGET_DIR/android/core-telemetry"
PROFILE="android"
FEATURES="uniffi,sqlite,http"

mkdir -p \
  "$OUT_DIR/kotlin" \
  "$OUT_DIR/jniLibs/arm64-v8a" \
  "$OUT_DIR/jniLibs/armeabi-v7a" \
  "$OUT_DIR/jniLibs/x86_64" \

function build() {
  echo "Building Rust library..."
  cargo build --target-dir "$TARGET_DIR" --features "$FEATURES"

  echo "Building Android so files for all architectures..."
  cargo ndk -t "armeabi-v7a" -t "arm64-v8a" -t "x86_64" build --features "$FEATURES" --target-dir "$TARGET_DIR" --profile "$PROFILE"

  echo "Generating Kotlin bindings from so file..."
  cargo run -p uniffi-bindgen --target-dir "$TARGET_DIR" -- \
    generate \
    --library "$TARGET_DIR/aarch64-linux-android/$PROFILE/${LIB_NAME}" \
    --language kotlin \
    --out-dir "$OUT_DIR/kotlin" \
    --no-format \
    --config uniffi.toml

  echo "Strip symbols from .so files..."
  "$STRIP_BIN" "$TARGET_DIR/aarch64-linux-android/$PROFILE/$LIB_NAME" -o "$OUT_DIR/jniLibs/arm64-v8a/$LIB_NAME"
  "$STRIP_BIN" "$TARGET_DIR/armv7-linux-androideabi/$PROFILE/$LIB_NAME" -o "$OUT_DIR/jniLibs/armeabi-v7a/$LIB_NAME"
  "$STRIP_BIN" "$TARGET_DIR/x86_64-linux-android/$PROFILE/$LIB_NAME" -o "$OUT_DIR/jniLibs/x86_64/$LIB_NAME"

  echo "Copying Android output..."
  mkdir -p "$ANDROID_MODULE_DIR/src/main/kotlin/me/proton"
  cp -R "$OUT_DIR/kotlin/me/proton/"* "$ANDROID_MODULE_DIR/src/main/kotlin/me/proton/"
  mkdir -p "$ANDROID_MODULE_DIR/src/main/jniLibs"
  cp -R "$OUT_DIR/jniLibs/"* "$ANDROID_MODULE_DIR/src/main/jniLibs/"

  echo "Build complete!"
}

function test() {
  echo "Running tests..."
  cd "$SCRIPT_DIR"
  cd ../../android
  ./gradlew :core-telemetry-ffi:connectedAndroidTest --rerun-tasks
  echo "Tests complete!"
}

function clean() {
  echo "Cleaning build artifacts..."
  cd "$SCRIPT_DIR"
  cargo clean --target-dir "$TARGET_DIR"

  echo "Cleaning Android generated files..."
  rm -rf "$OUT_DIR"
  rm -rf "$ANDROID_MODULE_DIR/src/main"
  echo "Clean complete!"
}

# Main script logic
case "${1:-build}" in
build)
        build
        ;;
test)
        test
        ;;
clean)
        clean
        ;;
*)
        echo "Error: Unknown command '$1'"
        exit 1
        ;;
esac
