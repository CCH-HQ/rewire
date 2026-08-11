#!/bin/sh

set -eu

home_dir=$1
node_runtime=$2
logs_dir=$4
node_image=$5

docker run --rm \
    --env HOME=/e2e/home \
    --env CLAUDE_CONFIG_DIR=/e2e/home/.claude \
    --volume "$home_dir:/e2e/home" \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" /runtime/node_modules/.bin/claude --version \
    > "$logs_dir/claude.version" 2> "$logs_dir/claude-version.stderr"

docker run --rm \
    --env HOME=/e2e/home \
    --env CLAUDE_CONFIG_DIR=/e2e/home/.claude \
    --volume "$home_dir:/e2e/home" \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" /runtime/node_modules/.bin/claude \
    -p 'Return exactly REWIRE_E2E_OK and nothing else.' \
    --model claude-sonnet-4-6 \
    --output-format json \
    > "$logs_dir/claude.stdout" 2> "$logs_dir/claude.stderr"
