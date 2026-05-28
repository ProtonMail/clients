#!/bin/bash

set -uo pipefail

# Usage: $0 rust_target config_path [output_dir]
# Builds a macOS-only universal framework for aarch64 (Apple Silicon) and x86_64 (Intel)

MIN_MACOS_VERSION="14.0"
PROFILE_MACOS="mail-macos"

# Must match where `cargo build` writes (respects CARGO_TARGET_DIR and `.cargo/config.toml`).
# A hardcoded `../../../target` breaks when the workspace uses another target directory.
BUILD_TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | jq -r '.target_directory')"
if [ -z "$BUILD_TARGET_DIR" ] || [ "$BUILD_TARGET_DIR" = "null" ]; then
    echo "error: could not resolve Cargo target directory (cargo metadata)." >&2
    exit 1
fi
echo "Cargo target directory: $BUILD_TARGET_DIR"

export LIBSQLITE3_FLAGS="-DNDEBUG=1 -DSQLITE_THREADSAFE=2\
 -DSQLITE_DEFAULT_FILE_PERMISSIONS=0600 -DSQLITE_SECURE_DELETE"

function check_exit {
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Command failed"
        exit $retVal
    fi
}

# gen_framework path name swift_include_dir crate_version
function gen_framework {
    local TARGET_DIR=$1
    local NAME=$2
    local FRAMEWORK_NAME="${NAME}_ffi.framework"
    local FRAMEWORK_PATH="$TARGET_DIR/$FRAMEWORK_NAME"
    local SWIFT_HEADER_DIR=$3
    local CRATE_VERSION=$4
    local NAME_PASCAL_CASE=$(echo "${NAME}_ffi" | perl -pe 's/(?:^|_)./uc($&)/ge;s/_//g')

    echo "$FRAMEWORK_PATH"

    find $TARGET_DIR -type d -name "$FRAMEWORK_NAME" -exec rm -rf {} \; || echo "rm failed"

    # Create deep bundle structure for macOS
    mkdir -p "$FRAMEWORK_PATH/Versions/A/Resources" "$FRAMEWORK_PATH/Versions/A/Headers" "$FRAMEWORK_PATH/Versions/A/Modules"
    check_exit

    # macOS framework Info.plist (in Resources subdirectory for deep bundle)
    cat > $FRAMEWORK_PATH/Versions/A/Resources/Info.plist << CATEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>me.proton.${NAME_PASCAL_CASE}</string>
    <key>CFBundleName</key>
    <string>${NAME}_ffi</string>
    <key>CFBundleVersion</key>
    <string>${CRATE_VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${CRATE_VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>${NAME}_ffi</string>
    <key>MinimumOSVersion</key>
    <string>${MIN_MACOS_VERSION}</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>macosx</string>
    </array>
</dict>
</plist>
CATEOF

    # Generate module map
    echo "framework module ${NAME}_ffi {" > $SWIFT_HEADER_DIR/module.tmp
    for header in $SWIFT_HEADER_DIR/*.h; do
        local header_name=$(basename "$header")
        echo "    header \"$header_name\"" >> $SWIFT_HEADER_DIR/module.tmp
        check_exit
    done

    echo "    export *" >> $SWIFT_HEADER_DIR/module.tmp
    echo "}" >> $SWIFT_HEADER_DIR/module.tmp

    cp $SWIFT_HEADER_DIR/*.h "$FRAMEWORK_PATH/Versions/A/Headers"
    check_exit

    cp $SWIFT_HEADER_DIR/module.tmp "$FRAMEWORK_PATH/Versions/A/Modules/module.modulemap"
    check_exit

    cp $TARGET_DIR/lib${NAME}.a "$FRAMEWORK_PATH/Versions/A/${NAME}_ffi"
    check_exit

    # Create symlinks for deep bundle structure
    (cd "$FRAMEWORK_PATH/Versions" && ln -s A Current)
    check_exit

    (cd "$FRAMEWORK_PATH" && ln -s Versions/Current/Resources Resources)
    check_exit
    (cd "$FRAMEWORK_PATH" && ln -s Versions/Current/Headers Headers)
    check_exit
    (cd "$FRAMEWORK_PATH" && ln -s Versions/Current/Modules Modules)
    check_exit
    (cd "$FRAMEWORK_PATH" && ln -s "Versions/Current/${NAME}_ffi" "${NAME}_ffi")
    check_exit
}

function gen_swift_package {
    local NAME=$2
    local FRAMEWORK_NAME=$3
    cat > $1/Package.swift << CATEOF
// swift-tools-version:5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "$NAME",
    platforms: [
      .macOS(.v14),
    ],
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "$NAME",
            targets: ["$NAME"]
        ),
    ],
    dependencies: [
        // Dependencies declare other packages that this package depends on.
        // .package(url: /* package url */, from: "1.0.0"),
    ],
    targets: [
        // Targets are the basic building blocks of a package. A target can define a module or a test suite.
        // Targets can depend on other targets in this package, and on products in packages this package depends on.
        .target(
            name: "$NAME",
            dependencies: ["${FRAMEWORK_NAME}_ffi"]
        ),
         .binaryTarget(name: "${FRAMEWORK_NAME}_ffi", path: "Sources/${FRAMEWORK_NAME}_ffi.xcframework"),
    ]
)

CATEOF
}

SO_SUFFIX="dylib"
TARGET=$1
TARGET_UNDERSCORE=$(echo $TARGET | tr "-" "_")
CONFIG_PATH="$2"
OUTPUT_DIR="$3"
if [ "$OUTPUT_DIR" == "" ]; then
    OUTPUT_DIR=$PWD
fi

# Optional features to enable
# - foundation_search: local body text search indexing
FEATURES="${CARGO_FEATURES:-}"

echo "Features = $FEATURES"

LIPO_INPUTS=""
echo "Building for macOS aarch64..."
MACOSX_DEPLOYMENT_TARGET=$MIN_MACOS_VERSION cargo build --profile $PROFILE_MACOS -p $TARGET --target aarch64-apple-darwin --features "$FEATURES"
check_exit
LIPO_INPUTS="$LIPO_INPUTS $BUILD_TARGET_DIR/aarch64-apple-darwin/$PROFILE_MACOS/lib${TARGET_UNDERSCORE}.a"

if [ -n "${AARCH64_ONLY:-}" ]; then
    echo "Skipping x86_64 build (AARCH64_ONLY is set)"
    XCFRAMEWORK_DIR="macos-arm64"
else
    echo "Building for macOS x86_64..."
    MACOSX_DEPLOYMENT_TARGET=$MIN_MACOS_VERSION cargo build --profile $PROFILE_MACOS -p $TARGET --target x86_64-apple-darwin --features "$FEATURES"
    check_exit
    LIPO_INPUTS="$LIPO_INPUTS $BUILD_TARGET_DIR/x86_64-apple-darwin/$PROFILE_MACOS/lib${TARGET_UNDERSCORE}.a"
    XCFRAMEWORK_DIR="macos-arm64_x86_64"
fi

mkdir -p $BUILD_TARGET_DIR/universal-apple-darwin/$PROFILE_MACOS
check_exit
echo "Creating fat universal binary..."
lipo $LIPO_INPUTS -create -output $BUILD_TARGET_DIR/universal-apple-darwin/$PROFILE_MACOS/lib${TARGET_UNDERSCORE}.a
check_exit

echo "Determining crate version"
CRATE_VERSION=$(cargo metadata --format-version 1 --no-deps | jq --arg target "$TARGET" '.packages[] | select(.name == $target) | .version' | jq -r)

# Create temporary directory for headers
TMP_DIR="/tmp/$(uuidgen)"
mkdir -p "$TMP_DIR/include"
check_exit

echo "Generating swift bindings"
# Generate against the aarch64 lib becase uniffi-bindgen can't read universal fat binaries
cargo run -p mail-uniffi-bindgen generate --library "$BUILD_TARGET_DIR/aarch64-apple-darwin/$PROFILE_MACOS/lib${TARGET_UNDERSCORE}.a" \
    --config "$CONFIG_PATH" --language swift --out-dir "$TMP_DIR/include"
check_exit

echo "Generating framework for macOS"
gen_framework "$BUILD_TARGET_DIR/universal-apple-darwin/$PROFILE_MACOS" $TARGET_UNDERSCORE "$TMP_DIR/include" $CRATE_VERSION

# Create Swift Package structure
PACKAGE_NAME="proton_app_uniffi"
SWIFT_PACKAGE_DIR=$OUTPUT_DIR/$PACKAGE_NAME
SWIFT_PACKAGE_SOURCES_DIR=$SWIFT_PACKAGE_DIR/Sources
XCFRAMEWORK_NAME="${TARGET_UNDERSCORE}_ffi.xcframework"
echo "Creating ${SWIFT_PACKAGE_SOURCES_DIR}/$XCFRAMEWORK_NAME"

mkdir -p $SWIFT_PACKAGE_SOURCES_DIR $SWIFT_PACKAGE_SOURCES_DIR/${TARGET_UNDERSCORE}
check_exit

gen_swift_package $SWIFT_PACKAGE_DIR $PACKAGE_NAME $TARGET_UNDERSCORE

if [[ -d "$SWIFT_PACKAGE_SOURCES_DIR/${XCFRAMEWORK_NAME}" ]]; then
    echo "Deleting previous artifact"
    rm -rf "$SWIFT_PACKAGE_SOURCES_DIR/${XCFRAMEWORK_NAME}"
    check_exit
fi

echo "Generating XCFramework for macOS"
xcodebuild -create-xcframework \
    -output "$SWIFT_PACKAGE_SOURCES_DIR/${XCFRAMEWORK_NAME}" \
    -framework "$BUILD_TARGET_DIR/universal-apple-darwin/$PROFILE_MACOS/${TARGET_UNDERSCORE}_ffi.framework"
check_exit

echo "Fixing symlinks in XCFramework (xcodebuild resolves them during packaging)"
XCFRAMEWORK_FRAMEWORK_PATH="$SWIFT_PACKAGE_SOURCES_DIR/${XCFRAMEWORK_NAME}/$XCFRAMEWORK_DIR/${TARGET_UNDERSCORE}_ffi.framework"
# Remove the resolved Current directory and recreate as symlink
rm -rf "$XCFRAMEWORK_FRAMEWORK_PATH/Versions/Current"
check_exit
(cd "$XCFRAMEWORK_FRAMEWORK_PATH/Versions" && ln -s A Current)
check_exit

# Recreate top-level symlinks (xcodebuild resolves these too)
rm -rf "$XCFRAMEWORK_FRAMEWORK_PATH/Headers" "$XCFRAMEWORK_FRAMEWORK_PATH/Modules" "$XCFRAMEWORK_FRAMEWORK_PATH/Resources" "$XCFRAMEWORK_FRAMEWORK_PATH/${TARGET_UNDERSCORE}_ffi"
check_exit
(cd "$XCFRAMEWORK_FRAMEWORK_PATH" && ln -s Versions/Current/Headers Headers)
check_exit
(cd "$XCFRAMEWORK_FRAMEWORK_PATH" && ln -s Versions/Current/Modules Modules)
check_exit
(cd "$XCFRAMEWORK_FRAMEWORK_PATH" && ln -s Versions/Current/Resources Resources)
check_exit
(cd "$XCFRAMEWORK_FRAMEWORK_PATH" && ln -s "Versions/Current/${TARGET_UNDERSCORE}_ffi" "${TARGET_UNDERSCORE}_ffi")
check_exit

echo "Changing folder name"
mv $SWIFT_PACKAGE_SOURCES_DIR/$TARGET_UNDERSCORE $SWIFT_PACKAGE_SOURCES_DIR/$PACKAGE_NAME

# Replace module imports with the correct module name
for header in $TMP_DIR/include/*.swift; do
    header_name=$(basename $header)
    header_name_only=$(basename $header ".swift")
    sed "s/${header_name_only}FFI/${TARGET_UNDERSCORE}_ffi/g" $header > $SWIFT_PACKAGE_SOURCES_DIR/${PACKAGE_NAME}/$header_name
    check_exit
done

echo "Cleaning up temporary directory"
rm -rf "$TMP_DIR"
check_exit

echo "Swift Package location: $SWIFT_PACKAGE_DIR"
echo "Package.swift: $SWIFT_PACKAGE_DIR/Package.swift"
echo "XCFramework: $SWIFT_PACKAGE_SOURCES_DIR/$XCFRAMEWORK_NAME"
