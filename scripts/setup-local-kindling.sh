#!/usr/bin/env bash
#
# setup-local-kindling.sh
# Writes .cargo/config.toml so cargo patches the crates.io kindling-mobi
# dependency with a local checkout. Useful for testing local kindling
# changes against lemma without publishing.
#
# Honors $KINDLING_PATH; otherwise tries a few sensible defaults.
#

set -e

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
config_dir="$repo_root/.cargo"
config_file="$config_dir/config.toml"

candidate=""
if [ -n "${KINDLING_PATH:-}" ]; then
    candidate="$KINDLING_PATH"
else
    for guess in \
        "$repo_root/../kindling" \
        "$HOME/Documents/kindling" \
        "$HOME/src/kindling" \
        "$HOME/code/kindling"
    do
        if [ -f "$guess/Cargo.toml" ]; then
            candidate="$guess"
            break
        fi
    done
fi

if [ -z "$candidate" ] || [ ! -f "$candidate/Cargo.toml" ]; then
    echo "kindling not found locally - falling back to crates.io"
    echo "(set KINDLING_PATH or place kindling at ../kindling or ~/Documents/kindling to override)"
    exit 0
fi

abs_candidate="$(cd "$candidate" && pwd)"

mkdir -p "$config_dir"

desired="[patch.crates-io]
kindling-mobi = { path = \"$abs_candidate\" }
"

if [ -f "$config_file" ]; then
    existing="$(cat "$config_file")"
    if [ "$existing" = "$desired" ]; then
        echo "local kindling patch already set at $abs_candidate"
        exit 0
    fi
fi

printf '%s' "$desired" > "$config_file"
echo "wrote $config_file pointing at $abs_candidate"
