#!/bin/sh

# Build io artifact locally and copy it to the right location in
# the ios repo
#
# Run from the root of the repository (e.g. project/mail/rust).
#
# Required: IOS_REPO_ROOT must be the root of the iOS mail app repo (the directory that
# contains ProtonPackages/proton_app_uniffi). Do not use $PATH — that is your shell PATH.
#
#   IOS_REPO_ROOT=/path/to/mail ./mail/mail-uniffi/ios/build-local.sh
#
# By default enables foundation search Cargo features (see CARGO_FEATURES below).

set -eo pipefail

if [ -z "$IOS_REPO_ROOT" ]; then
    echo "error: IOS_REPO_ROOT is not set." >&2
    echo "Set it to your Proton Mail iOS checkout (folder containing ProtonPackages/proton_app_uniffi)." >&2
    echo "Example: IOS_REPO_ROOT=\"\$HOME/PROTON/mail\" $0" >&2
    exit 1
fi

if [ ! -d "$IOS_REPO_ROOT/ProtonPackages/proton_app_uniffi" ]; then
    echo "error: IOS_REPO_ROOT does not look like the mail iOS repo:" >&2
    echo "  missing: $IOS_REPO_ROOT/ProtonPackages/proton_app_uniffi" >&2
    exit 1
fi

# `rust-build/build_ios_framework_uniffi.sh` reads CARGO_FEATURES (default there is only
# `stdout_logging`). For local iOS SDK builds, turn on foundation search by default:
#   - foundation_search: local body indexing + search UI hooks (e.g. scroller highlighting)
#   - foundation_search_index_timing: index timing instrumentation
#   - foundation_search_lab_harness: historic load orchestration + perf/fixture APIs (UniFFI)
# Override for a minimal build: CARGO_FEATURES=stdout_logging ./mail/mail-uniffi/ios/build-local.sh
: "${CARGO_FEATURES:=stdout_logging,foundation_search,foundation_search_index_timing,foundation_search_lab_harness}"
export CARGO_FEATURES

TMP_DIR="/tmp/$(uuidgen)"
PROFILE="${1:-}"

if [ -n "$PROFILE" ]; then
    echo "Building with profile: $PROFILE"
    rust-build/build_ios_framework_uniffi.sh mail-uniffi ./mail/mail-uniffi/uniffi.toml $TMP_DIR $PROFILE
else
    echo "Building with default profile (release)"
    rust-build/build_ios_framework_uniffi.sh mail-uniffi ./mail/mail-uniffi/uniffi.toml $TMP_DIR
fi

CRATE_VERSION=$(cargo pkgid --manifest-path=./mail/mail-uniffi/Cargo.toml | cut -d "#" -f2)

cp -r $TMP_DIR/$CRATE_VERSION/* $IOS_REPO_ROOT/ProtonPackages/proton_app_uniffi/
