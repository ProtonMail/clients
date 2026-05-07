#!/bin/bash

set -eo pipefail

cd ./mail/mail-uniffi/android/

# Set version for maven lib release
CRATE_VERSION=$(cargo metadata --locked --format-version 1 --no-deps | jq '.packages[] | select(.name == "mail-uniffi") | .version' | jq -r)
sed -i "s%    version = .*%    version = \"$CRATE_VERSION\"%g" "./lib/build.gradle.kts"

# Build and Publish lib to maven
./gradlew publishAllPublicationsToMavenCentralRepository
