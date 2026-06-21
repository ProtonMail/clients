#!/usr/bin/env bash
set -eu

# usage arg0 target config_path out_dir
# Run from the monorepo root workspace (this script lives under project/mail/rust/).

# Assert go version
GO_VERSION="$(go version | awk '{print $3}' | sed 's/^go//')"
if [[ "$GO_VERSION" != "1.22.12" ]]; then
    echo "INVALID go version: $GO_VERSION"
    exit 1
fi
echo "Go version = $GO_VERSION"

# https://android.googlesource.com/platform/external/sqlite/+/refs/heads/main/dist/Android.bp
export LIBSQLITE3_FLAGS="-DNDEBUG=1 -DSQLITE_POWERSAFE_OVERWRITE=1 -DSQLITE_THREADSAFE=2\
 -DSQLITE_DEFAULT_FILE_PERMISSIONS=0600 -DSQLITE_SECURE_DELETE -DSQLITE_ENABLE_BATCH_ATOMIC_WRITE"

TARGET=$1
CONFIG_PATH=$2
OUT_DIR=$3
PROFILE="mail-android"
LIB_NAME="lib$(echo $1 | tr '-' '_').so"

# Space-separated cargo-ndk ABI names. Default: all release ABIs. For faster local
# iteration (physical device or arm64 emulator only): MAIL_ANDROID_ABIS=arm64-v8a
MAIL_ANDROID_ABIS="${MAIL_ANDROID_ABIS:-armeabi-v7a arm64-v8a x86_64}"

rust_triple_for_abi() {
    case "$1" in
    arm64-v8a) echo aarch64-linux-android ;;
    armeabi-v7a) echo armv7-linux-androideabi ;;
    x86_64) echo x86_64-linux-android ;;
    *)
        echo "Unknown MAIL_ANDROID_ABIS entry '$1' (expected arm64-v8a, armeabi-v7a, or x86_64)" >&2
        exit 1
        ;;
    esac
}

# UniFFI bindgen loads one built .so; prefer arm64 when present, else first listed ABI.
bindgen_abi() {
    for a in $MAIL_ANDROID_ABIS; do
        if [[ "$a" == "arm64-v8a" ]]; then
            echo arm64-v8a
            return
        fi
    done
    echo $MAIL_ANDROID_ABIS | awk '{print $1}'
}

# Honour global target-dir (e.g. ~/.cargo/target in ~/.cargo/config.toml). The old
# ../../../target guess breaks bindgen and cp when Cargo writes elsewhere.
if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    BUILD_TARGET_DIR="${CARGO_TARGET_DIR}"
else
    BUILD_TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | jq -r '.target_directory')"
fi

mkdir -p "$OUT_DIR/java"
rm -rf "$OUT_DIR/jniLibs"
for abi in $MAIL_ANDROID_ABIS; do
    mkdir -p "$OUT_DIR/jniLibs/$abi"
done

# Optional features to enable
# - foundation_search: local body text search indexing
FEATURES="${CARGO_FEATURES:-}"

echo "Features = $FEATURES"
echo "MAIL_ANDROID_ABIS = $MAIL_ANDROID_ABIS"

NDK_TARGETS=()
for abi in $MAIL_ANDROID_ABIS; do
    NDK_TARGETS+=(-t "$abi")
done

# Build project
RUSTFLAGS="--cfg forcego" cargo ndk "${NDK_TARGETS[@]}" build --profile $PROFILE -p $TARGET --features "$FEATURES"

BINDGEN_TRIPLE=$(rust_triple_for_abi "$(bindgen_abi)")

# Generate bindings
cargo run \
    --release \
    -p mail-uniffi-bindgen generate \
    --library "$BUILD_TARGET_DIR/$BINDGEN_TRIPLE/$PROFILE/${LIB_NAME}" \
    --language kotlin \
    --config ${CONFIG_PATH} \
    --out-dir $OUT_DIR/java \
    --no-format

for abi in $MAIL_ANDROID_ABIS; do
    triple=$(rust_triple_for_abi "$abi")
    cp "$BUILD_TARGET_DIR/$triple/$PROFILE/$LIB_NAME" "$OUT_DIR/jniLibs/$abi/$LIB_NAME"
done

PGP_SYS_LIB="libgopenpgp-sys.so"
for abi in $MAIL_ANDROID_ABIS; do
    triple=$(rust_triple_for_abi "$abi")
    if [[ -f "$BUILD_TARGET_DIR/$triple/$PROFILE/$PGP_SYS_LIB" ]]; then
        cp "$BUILD_TARGET_DIR/$triple/$PROFILE/$PGP_SYS_LIB" "$OUT_DIR/jniLibs/$abi/$PGP_SYS_LIB"
    fi
done
