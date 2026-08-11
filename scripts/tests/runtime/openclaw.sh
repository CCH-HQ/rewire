#!/bin/sh

set -eu

home_dir=$1
node_runtime=$2
logs_dir=$4
node_image=$5

docker run --rm \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" /runtime/node_modules/.bin/openclaw --version \
    > "$logs_dir/openclaw.version" 2> "$logs_dir/openclaw-version.stderr"

# File SecretRefs are resolved by OpenClaw's active Gateway snapshot. The deterministic control
# token authenticates only this loopback test Gateway and is unrelated to the model API token.
docker run --rm \
    --env HOME=/e2e/home \
    --env OPENCLAW_STATE_DIR=/e2e/home/.openclaw \
    --env OPENCLAW_GATEWAY_TOKEN=rewire-e2e-control-token \
    --volume "$home_dir:/e2e/home" \
    --volume "$logs_dir:/e2e/logs" \
    --volume "$node_runtime:/runtime:ro" \
    "$node_image" sh -eu -c '
        /runtime/node_modules/.bin/openclaw --log-level warn gateway run \
            --auth token --bind loopback --allow-unconfigured \
            > /e2e/logs/openclaw-gateway.stdout \
            2> /e2e/logs/openclaw-gateway.stderr &
        gateway_pid=$!
        trap "kill $gateway_pid 2>/dev/null || true" EXIT INT TERM

        attempt=0
        until node -e '\''const net=require("net");const s=net.connect(18789,"127.0.0.1",()=>{s.end();process.exit(0)});s.on("error",()=>process.exit(1))'\''
        do
            attempt=$((attempt + 1))
            [ "$attempt" -lt 30 ] || exit 70
            sleep 1
        done

        /runtime/node_modules/.bin/openclaw --log-level warn agent \
            --json \
            --session-id rewire-e2e \
            --message "Return exactly REWIRE_E2E_OK and nothing else." \
            > /e2e/logs/openclaw.stdout \
            2> /e2e/logs/openclaw.stderr
    '
