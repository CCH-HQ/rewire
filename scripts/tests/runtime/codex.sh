#!/bin/sh

set -eu

home_dir=$1
node_runtime=$2
logs_dir=$4
node_image=$5

docker run --rm \
    --env HOME=/e2e/home \
    --env CODEX_HOME=/e2e/home/.codex \
    --volume "$home_dir:/e2e/home" \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" /runtime/node_modules/.bin/codex --version \
    > "$logs_dir/codex.version" 2> "$logs_dir/codex-version.stderr"

docker run --rm \
    --env HOME=/e2e/home \
    --env CODEX_HOME=/e2e/home/.codex \
    --volume "$home_dir:/e2e/home" \
    --volume "$logs_dir:/e2e/logs" \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" /runtime/node_modules/.bin/codex exec \
    --profile rewire \
    --model gpt-5.5 \
    --skip-git-repo-check \
    --ephemeral \
    --sandbox read-only \
    --output-last-message /e2e/logs/codex.last \
    'Return exactly REWIRE_E2E_OK and nothing else.' \
    > "$logs_dir/codex.stdout" 2> "$logs_dir/codex.stderr"
