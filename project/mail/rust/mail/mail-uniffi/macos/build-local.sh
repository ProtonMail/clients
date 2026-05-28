#!/bin/sh

# Build macOS artifact locally and output to the specified folder (or ./target/build/macos/proton_app_uniffi is not specified).
#
# Run from the root of the repository
#
# ./mail/mail-uniffi/macos/build-local.sh

set -eo pipefail

TMP_DIR="/tmp/$(uuidgen)"

rust-build/build_macos_framework_uniffi.sh mail-uniffi ./mail/mail-uniffi/uniffi.toml $TMP_DIR

PACKAGE_NAME="proton_app_uniffi"
OUTPUT_DIR="./target/build/macos/${PACKAGE_NAME}"
if [[ $# -gt 0 ]]; then
    OUTPUT_DIR="$1"
fi
echo "OUTPUT_DIR is ${OUTPUT_DIR}"

# Remove old output if it exists
rm -rf $OUTPUT_DIR

mkdir -p "$(dirname $OUTPUT_DIR)"

cp -a $TMP_DIR/$PACKAGE_NAME $OUTPUT_DIR

echo "Output location: $OUTPUT_DIR"
