#!/bin/sh

set -eu

home_dir=$1
node_runtime=$2
logs_dir=$4
node_image=$5

docker run --rm \
    --env HOME=/e2e/home \
    --env XDG_CONFIG_HOME=/e2e/home/.config \
    --env OPENCODE_CONFIG_DIR=/e2e/home/.config/opencode \
    --volume "$home_dir:/e2e/home" \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" /runtime/node_modules/.bin/opencode --version \
    > "$logs_dir/opencode.version" 2> "$logs_dir/opencode-version.stderr"

docker run --rm \
    --env HOME=/e2e/home \
    --env XDG_CONFIG_HOME=/e2e/home/.config \
    --env OPENCODE_CONFIG_DIR=/e2e/home/.config/opencode \
    --volume "$home_dir:/e2e/home" \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" /runtime/node_modules/.bin/opencode run \
    --pure \
    --model anthropic/claude-sonnet-4-6 \
    --format json \
    'Return exactly REWIRE_E2E_OK and nothing else.' \
    > "$logs_dir/opencode.stdout" 2> "$logs_dir/opencode.stderr"
