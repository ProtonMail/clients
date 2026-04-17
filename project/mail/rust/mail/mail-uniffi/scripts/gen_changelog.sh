#!/bin/bash

set -euo pipefail

INIT=mail-uniffi-v0.163.4

uv run --project scripts/changelog changelog "$@" --init $INIT --path project/mail/rust --repo ../../../ --only "^mail-uniffi-" >mail/mail-uniffi/CHANGELOG.md

cat mail/mail-uniffi/CHANGELOG.old.md >>mail/mail-uniffi/CHANGELOG.md
