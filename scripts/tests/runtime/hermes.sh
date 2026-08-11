#!/bin/sh

set -eu

home_dir=$1
hermes_runtime=$3
logs_dir=$4
python_image=$6

docker run --rm \
    --volume "$hermes_runtime:/runtime:ro" \
    "$python_image" /runtime/venv/bin/hermes --version \
    > "$logs_dir/hermes.version" 2> "$logs_dir/hermes-version.stderr"

docker run --rm \
    --env HOME=/e2e/home \
    --env HERMES_HOME=/e2e/home/.hermes \
    --volume "$home_dir:/e2e/home" \
    --volume "$logs_dir:/e2e/logs" \
    --volume "$hermes_runtime:/runtime:ro" \
    "$python_image" /runtime/venv/bin/hermes \
    -z 'Return exactly REWIRE_E2E_OK and nothing else.' \
    --model claude-sonnet-4-6 \
    --provider rewire \
    --ignore-rules \
    --usage-file /e2e/logs/hermes-usage.json \
    > "$logs_dir/hermes.stdout" 2> "$logs_dir/hermes.stderr"
