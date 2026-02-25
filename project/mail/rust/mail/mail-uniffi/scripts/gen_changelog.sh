#!/bin/bash

set -euo pipefail

ROOT=project/mail/rust
INIT=mail-uniffi-v0.163.4

uv run --project $ROOT/scripts/changelog changelog "$@" --init $INIT --path $ROOT
