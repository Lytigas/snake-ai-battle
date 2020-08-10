#!/usr/bin/env bash

set -euxo pipefail

tmppipe=$(mktemp -u)
mkfifo "$tmppipe"

# put how you invoke your bot here, and how you invoke client-adapter here
python3 bots/bot_txt.py < "$tmppipe" | ./target/debug/client-adapter > "$tmppipe"
