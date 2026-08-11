#!/bin/sh

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
key_file=${REWIRE_E2E_KEY_FILE:-$root/tmp/key}
domain_file=${REWIRE_E2E_DOMAIN_FILE:-$root/tmp/domain}
image=${REWIRE_E2E_IMAGE:-rust:1.94.0-bookworm}
skip_api_probe=0

usage() {
    cat <<'EOF'
Build a Linux release asset, install it over HTTP in Docker, and configure all clients.

Usage:
  install-docker-e2e.sh [OPTIONS]

Options:
  --key-file <PATH>     API token file (default: tmp/key)
  --domain-file <PATH>  API base URL file (default: tmp/domain)
  --image <IMAGE>       Builder/client image (default: rust:1.94.0-bookworm)
  --skip-api-probe      Skip the authenticated /v1/models compatibility probe
  -h, --help            Print this help

The API token is mounted read-only and passed to Rewire through stdin. It is never
placed in a Docker command argument, environment variable, image layer, or test log.
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
        --key-file)
            require_value "$1" "$#"
            key_file=$2
            shift 2
            ;;
        --domain-file)
            require_value "$1" "$#"
            domain_file=$2
            shift 2
            ;;
        --image)
            require_value "$1" "$#"
            image=$2
            shift 2
            ;;
        --skip-api-probe)
            skip_api_probe=1
            shift
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

command -v docker >/dev/null 2>&1 || {
    printf 'docker is required\n' >&2
    exit 1
}
[ -s "$key_file" ] || {
    printf 'API token file is missing or empty: %s\n' "$key_file" >&2
    exit 1
}
[ -s "$domain_file" ] || {
    printf 'API domain file is missing or empty: %s\n' "$domain_file" >&2
    exit 1
}

domain=$(sed -n '1p' "$domain_file")
case "$domain" in
    http://* | https://*) ;;
    *)
        printf 'API domain must be an HTTP(S) URL\n' >&2
        exit 1
        ;;
esac

tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/rewire-docker-e2e.XXXXXX")
network=rewire-e2e-$$
server=rewire-e2e-assets-$$
network_created=0
server_started=0
cleanup() {
    if [ "$server_started" -eq 1 ]; then
        docker rm --force "$server" >/dev/null 2>&1 || true
    fi
    if [ "$network_created" -eq 1 ]; then
        docker network rm "$network" >/dev/null 2>&1 || true
    fi
    rm -rf "$tmpdir"
}
trap cleanup 0 1 2 3 15

assets=$tmpdir/assets
target_dir=$tmpdir/target
home_dir=$tmpdir/home
install_dir=$tmpdir/install
logs=$tmpdir/logs
empty_tmp=$tmpdir/empty-tmp
mkdir -p "$assets" "$target_dir" "$home_dir" "$install_dir" "$logs" "$empty_tmp"

commit=$(git -C "$root" rev-parse --short=12 HEAD 2>/dev/null || printf unknown)
printf 'Building the release archive in %s...\n' "$image"
docker run --rm \
    --env CARGO_TARGET_DIR=/target \
    --env REWIRE_GIT_COMMIT="$commit" \
    --volume "$root:/workspace:ro" \
    --volume "$empty_tmp:/workspace/tmp:ro" \
    --volume "$assets:/dist" \
    --volume "$target_dir:/target" \
    --workdir /workspace \
    "$image" sh -eu -c '
        cargo build --locked --release
        target=$(rustc -vV | sed -n "s/^host: //p")
        case "$target" in
            x86_64-unknown-linux-gnu | aarch64-unknown-linux-gnu) ;;
            *) printf "unsupported Docker Rust host: %s\n" "$target" >&2; exit 1 ;;
        esac
        package=/dist/package
        mkdir "$package"
        cp /target/release/rewire "$package/rewire"
        cp README.md CHANGELOG.md LICENSE "$package/"
        archive="rewire-$target.tar.gz"
        tar -C "$package" -czf "/dist/$archive" .
        (cd /dist && sha256sum "$archive" > SHA256SUMS)
        printf "%s" "$target" > /dist/TARGET
    '

docker network create "$network" >/dev/null
network_created=1
docker run --detach --rm \
    --name "$server" \
    --network "$network" \
    --network-alias assets \
    --volume "$assets:/srv:ro" \
    --volume "$logs:/logs" \
    "$image" sh -c \
    'cd /srv && exec python3 -u -m http.server 8080 > /logs/http.log 2>&1' \
    >/dev/null
server_started=1

attempt=0
until docker run --rm --network "$network" "$image" \
    curl --fail --silent --show-error http://assets:8080/SHA256SUMS >/dev/null 2>&1
do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 20 ] || {
        printf 'asset server did not become ready\n' >&2
        exit 1
    }
    sleep 1
done

printf 'Installing over HTTP and configuring five isolated clients...\n'
set +e
docker run --rm --interactive \
    --network "$network" \
    --volume "$root/scripts/install.sh:/install.sh:ro" \
    --volume "$domain_file:/run/rewire/domain:ro" \
    --volume "$home_dir:/e2e/home" \
    --volume "$install_dir:/e2e/install" \
    "$image" sh -eu -c '
        base_url=$(cat /run/rewire/domain)
        exec sh /install.sh \
            --asset-base-url http://assets:8080 \
            --install-dir /e2e/install \
            -- \
            --baseurl "$base_url" \
            --token-stdin \
            --client claude,codex,opencode,hermes,openclaw \
            --model claude-sonnet-4-6 \
            --model-name "Claude Sonnet 4.6" \
            --sdk anthropic \
            --home /e2e/home \
            --yes \
            --json
    ' < "$key_file" > "$logs/apply.stdout" 2> "$logs/apply.stderr"
apply_status=$?
set -e

if grep -F -f "$key_file" "$logs/apply.stdout" "$logs/apply.stderr" >/dev/null 2>&1; then
    printf 'API token leaked into installer or Rewire output\n' >&2
    exit 1
fi
[ "$apply_status" -eq 0 ] || {
    printf 'installer/configuration command failed; output was withheld to protect credentials\n' >&2
    exit "$apply_status"
}

# A second plan over the generated files proves every adapter can parse its own output and that
# the complete operation is idempotent. The credential again travels only through standard input.
docker run --rm --interactive \
    --volume "$domain_file:/run/rewire/domain:ro" \
    --volume "$home_dir:/e2e/home" \
    --volume "$install_dir:/e2e/install:ro" \
    "$image" sh -eu -c '
        base_url=$(cat /run/rewire/domain)
        exec /e2e/install/rewire \
            --baseurl "$base_url" \
            --token-stdin \
            --client claude,codex,opencode,hermes,openclaw \
            --model claude-sonnet-4-6 \
            --model-name "Claude Sonnet 4.6" \
            --sdk anthropic \
            --home /e2e/home \
            --dry-run \
            --json
    ' < "$key_file" > "$logs/idempotent.stdout" 2> "$logs/idempotent.stderr"

if grep -F -f "$key_file" "$logs/idempotent.stdout" "$logs/idempotent.stderr" >/dev/null 2>&1; then
    printf 'API token leaked into the idempotency plan output\n' >&2
    exit 1
fi

verify_args=
if [ "$skip_api_probe" -eq 1 ]; then
    verify_args=--skip-api-probe
fi
docker run --rm \
    --volume "$root/scripts/tests/verify-docker-e2e.py:/verify.py:ro" \
    --volume "$key_file:/run/rewire/key:ro" \
    --volume "$domain_file:/run/rewire/domain:ro" \
    --volume "$assets:/e2e/assets:ro" \
    --volume "$home_dir:/e2e/home:ro" \
    --volume "$install_dir:/e2e/install:ro" \
    --volume "$logs:/e2e/logs:ro" \
    "$image" python3 /verify.py $verify_args

printf 'Docker installer E2E passed.\n'
