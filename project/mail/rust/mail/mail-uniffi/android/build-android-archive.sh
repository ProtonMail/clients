#!/bin/bash

set -eo pipefail

cd ./mail/mail-uniffi/android/
./gradlew :lib:assembleRelease
