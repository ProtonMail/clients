#/bin/bash
set -eu

# usage arg0 target config_path out_dir

# https://android.googlesource.com/platform/external/sqlite/+/refs/heads/main/dist/Android.bp
export LIBSQLITE3_FLAGS="-DNDEBUG=1 -DSQLITE_POWERSAFE_OVERWRITE=1 -DSQLITE_THREADSAFE=2\
 -DSQLITE_DEFAULT_FILE_PERMISSIONS=0600 -DSQLITE_SECURE_DELETE -DSQLITE_ENABLE_BATCH_ATOMIC_WRITE"

TARGET=$1
CONFIG_PATH=$2
OUT_DIR=$3
PROFILE="mail-android"
LIB_NAME="lib$(echo $1 | tr '-' '_').so"
BUILD_TARGET_DIR="../../../target"

mkdir -p $OUT_DIR/java \
  $OUT_DIR/jniLibs/arm64-v8a \
  $OUT_DIR/jniLibs/armeabi-v7a \
  $OUT_DIR/jniLibs/x86_64

# Optional features to enable
# - foundation_search: local body text search indexing
FEATURES="${CARGO_FEATURES:-}"

echo "Features = $FEATURES"
# Build project
RUSTFLAGS="--cfg forcego" cargo ndk -t "armeabi-v7a" -t "arm64-v8a" -t "x86_64" build --profile $PROFILE -p $TARGET --features "$FEATURES"

# Generate bindings
cargo run \
  --release \
  -p mail-uniffi-bindgen generate \
  --library $BUILD_TARGET_DIR/aarch64-linux-android/$PROFILE/${LIB_NAME} \
  --language kotlin \
  --config ${CONFIG_PATH} \
  --out-dir $OUT_DIR/java \
  --no-format

# Strip symbols
OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
cp $BUILD_TARGET_DIR/aarch64-linux-android/$PROFILE/$LIB_NAME $OUT_DIR/jniLibs/arm64-v8a/$LIB_NAME
cp $BUILD_TARGET_DIR/armv7-linux-androideabi/$PROFILE/$LIB_NAME $OUT_DIR/jniLibs/armeabi-v7a/$LIB_NAME
cp $BUILD_TARGET_DIR/x86_64-linux-android/$PROFILE/$LIB_NAME $OUT_DIR/jniLibs/x86_64/$LIB_NAME

PGP_SYS_LIB="libgopenpgp-sys.so"
if [[ -f "$BUILD_TARGET_DIR/aarch64-linux-android/$PROFILE/$PGP_SYS_LIB" ]]; then
  cp $BUILD_TARGET_DIR/aarch64-linux-android/$PROFILE/$PGP_SYS_LIB $OUT_DIR/jniLibs/arm64-v8a/$PGP_SYS_LIB
  cp $BUILD_TARGET_DIR/armv7-linux-androideabi/$PROFILE/$PGP_SYS_LIB $OUT_DIR/jniLibs/armeabi-v7a/$PGP_SYS_LIB
  cp $BUILD_TARGET_DIR/x86_64-linux-android/$PROFILE/$PGP_SYS_LIB $OUT_DIR/jniLibs/x86_64/$PGP_SYS_LIB
fi
