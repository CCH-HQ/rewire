#!/bin/sh

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
home_dir=
key_file=
logs_dir=
work_dir=
node_image=${REWIRE_E2E_NODE_IMAGE:-node:24-bookworm}
python_image=${REWIRE_E2E_PYTHON_IMAGE:-python:3.13-bookworm}

usage() {
    cat <<'EOF'
Install pinned official clients in Docker and run one real model turn through each.

Usage:
  client-runtime-e2e.sh --home-dir <PATH> --key-file <PATH> --logs-dir <PATH> --work-dir <PATH>

Options:
  --home-dir <PATH>     Isolated Home already configured by Rewire
  --key-file <PATH>     API token file used only for post-run leak scanning
  --logs-dir <PATH>     Destination for client stdout, stderr, and usage records
  --work-dir <PATH>     Disposable client installation directory
  --node-image <IMAGE>  Node image (default: node:24-bookworm)
  --python-image <IMG>  Python image (default: python:3.13-bookworm)
  -h, --help            Print this help

The API token is read by clients from Rewire-managed configuration. This harness never places it
in a Docker argument, environment variable, image layer, or generated command line.
EOF
}

require_value() {
    option=$1
    count=$2
    [ "$count" -ge 2 ] || {
        printf '%s requires a value\n' "$option" >&2
        exit 2
    }
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --home-dir | --key-file | --logs-dir | --work-dir | --node-image | --python-image)
            require_value "$1" "$#"
            case "$1" in
                --home-dir) home_dir=$2 ;;
                --key-file) key_file=$2 ;;
                --logs-dir) logs_dir=$2 ;;
                --work-dir) work_dir=$2 ;;
                --node-image) node_image=$2 ;;
                --python-image) python_image=$2 ;;
            esac
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

for required in home_dir key_file logs_dir work_dir; do
    eval "value=\${$required}"
    [ -n "$value" ] || {
        printf '%s is required\n' "$(printf '%s' "$required" | tr '_' '-')" >&2
        exit 2
    }
done

command -v docker >/dev/null 2>&1 || {
    printf 'docker is required\n' >&2
    exit 1
}
[ -d "$home_dir" ] || {
    printf 'configured Home does not exist: %s\n' "$home_dir" >&2
    exit 1
}
[ -s "$key_file" ] || {
    printf 'API token file is missing or empty: %s\n' "$key_file" >&2
    exit 1
}

node_runtime=$work_dir/node
hermes_runtime=$work_dir/hermes
mkdir -p "$logs_dir" "$node_runtime" "$hermes_runtime"

printf 'Installing pinned official client CLIs...\n'
docker run --rm \
    --volume "$node_runtime:/runtime" \
    "$node_image" npm install --prefix /runtime --no-audit --no-fund \
    @anthropic-ai/claude-code@2.1.186 \
    @openai/codex@0.147.0 \
    opencode-ai@1.18.16 \
    openclaw@2026.7.1-2 \
    > "$logs_dir/npm-install.stdout" 2> "$logs_dir/npm-install.stderr"

docker run --rm \
    --volume "$hermes_runtime:/runtime" \
    "$python_image" sh -eu -c '
        python -m venv /runtime/venv
        /runtime/venv/bin/pip install --disable-pip-version-check --no-cache-dir \
            "hermes-agent[anthropic]==0.19.0"
    ' > "$logs_dir/hermes-install.stdout" 2> "$logs_dir/hermes-install.stderr"

for client in claude codex opencode hermes openclaw; do
    printf 'Running %s through the configured gateway...\n' "$client"
    if ! sh "$root/scripts/tests/runtime/$client.sh" \
        "$home_dir" "$node_runtime" "$hermes_runtime" "$logs_dir" \
        "$node_image" "$python_image"
    then
        if grep -F -f "$key_file" "$logs_dir"/* >/dev/null 2>&1; then
            printf 'API token leaked into client runtime output\n' >&2
        else
            printf '%s runtime call failed; sanitized output follows:\n' "$client" >&2
            for output in \
                "$logs_dir/$client.stdout" \
                "$logs_dir/$client.stderr" \
                "$logs_dir/$client-usage.json"
            do
                if [ -s "$output" ]; then
                    printf '%s\n' "--- $(basename "$output") ---" >&2
                    tail -n 80 "$output" >&2
                fi
            done
        fi
        exit 1
    fi
done

docker run --rm \
    --volume "$root/scripts/tests/verify-client-runtime.py:/verify.py:ro" \
    --volume "$key_file:/run/rewire/key:ro" \
    --volume "$logs_dir:/e2e/logs:ro" \
    "$python_image" python3 /verify.py

printf 'All five configured clients completed a real model turn.\n'
