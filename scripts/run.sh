#!/bin/sh

set -eu

REPOSITORY="CCH-HQ/rewire"
release=${REWIRE_RELEASE:-latest}
asset_base_url=${REWIRE_ASSET_BASE_URL:-}
download_url=${REWIRE_DOWNLOAD_URL:-}
checksum_url=${REWIRE_CHECKSUM_URL:-}
expected_sha256=${REWIRE_SHA256:-}
installer_url=${REWIRE_INSTALLER_URL:-}
asset_base_option_set=0
download_option_set=0
checksum_option_set=0
sha256_option_set=0

fail() {
    printf 'rewire runner: %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Download, verify, and run Rewire without installing it.

Usage:
  run.sh [RUNNER OPTIONS] [--] [REWIRE ARGUMENTS...]

Runner options:
  --release <VERSION>       Release to run, for example 0.0.1 or v0.0.1
                            (default: latest)
  --asset-base-url <VALUE>  Release asset URL or local fixture/mirror directory
  --download-url <URL>      Exact platform archive URL or local file
  --checksum-url <URL>      Exact SHA256SUMS URL or local file
  --sha256 <DIGEST>         Expected archive SHA-256 instead of SHA256SUMS
  --installer-url <URL>     install.sh URL or local file used by this runner
  -h, --help                Print this help

The verified binary is staged in a private temporary directory and removed
after it exits. With no Rewire arguments, the runner starts `rewire configure`.
All arguments after `--` are passed to Rewire unchanged.
EOF
}

require_value() {
    option=$1
    count=$2
    [ "$count" -ge 2 ] || fail "$option requires a value"
}

should_attach_terminal_input() {
    [ ! -t 0 ] && [ -t 1 ] || return 1
    ( : < /dev/tty ) 2>/dev/null || return 1
    for argument in "$@"; do
        case "$argument" in
            --token-stdin | --non-interactive) return 1 ;;
        esac
    done
    return 0
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --release)
            require_value "$1" "$#"
            release=$2
            shift 2
            ;;
        --asset-base-url)
            require_value "$1" "$#"
            [ "$download_option_set" -eq 0 ] \
                || fail "--asset-base-url conflicts with --download-url"
            asset_base_url=$2
            download_url=
            asset_base_option_set=1
            shift 2
            ;;
        --download-url)
            require_value "$1" "$#"
            [ "$asset_base_option_set" -eq 0 ] \
                || fail "--download-url conflicts with --asset-base-url"
            download_url=$2
            asset_base_url=
            download_option_set=1
            shift 2
            ;;
        --checksum-url)
            require_value "$1" "$#"
            [ "$sha256_option_set" -eq 0 ] \
                || fail "--checksum-url conflicts with --sha256"
            checksum_url=$2
            expected_sha256=
            checksum_option_set=1
            shift 2
            ;;
        --sha256)
            require_value "$1" "$#"
            [ "$checksum_option_set" -eq 0 ] \
                || fail "--sha256 conflicts with --checksum-url"
            expected_sha256=$2
            checksum_url=
            sha256_option_set=1
            shift 2
            ;;
        --installer-url)
            require_value "$1" "$#"
            installer_url=$2
            shift 2
            ;;
        --install-dir)
            fail "--install-dir is reserved by the run-only entrypoint"
            ;;
        --no-run | --quiet)
            fail "$1 is an installer-only option"
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        *)
            break
            ;;
    esac
done

[ -z "$download_url" ] || [ -z "$asset_base_url" ] \
    || fail "--download-url conflicts with --asset-base-url"
[ -z "$expected_sha256" ] || [ -z "$checksum_url" ] \
    || fail "--sha256 conflicts with --checksum-url"

download_installer() {
    source=$1
    destination=$2
    if [ -f "$source" ]; then
        cp "$source" "$destination" || fail "could not copy installer from $source"
        return
    fi

    if command -v curl >/dev/null 2>&1; then
        case "$source" in
            https://*)
                curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
                    --retry 3 --retry-delay 1 --output "$destination" "$source" \
                    || fail "could not download installer from $source"
                ;;
            *)
                curl --fail --location --silent --show-error --retry 3 --retry-delay 1 \
                    --output "$destination" "$source" \
                    || fail "could not download installer from $source"
                ;;
        esac
    elif command -v wget >/dev/null 2>&1; then
        wget -q --tries=3 --output-document="$destination" "$source" \
            || fail "could not download installer from $source"
    else
        fail "curl or wget is required to download install.sh"
    fi
}

# Resolve a sibling installer even when run.sh itself was found through PATH.
script_path=$0
case "$script_path" in
    */*) ;;
    *)
        resolved=$(command -v "$script_path" 2>/dev/null) || resolved=$script_path
        script_path=$resolved
        ;;
esac
script_dir=$(CDPATH='' cd -- "$(dirname -- "$script_path")" && pwd)

umask 077
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/rewire-run.XXXXXX") \
    || fail "could not create a temporary directory"
cleanup() {
    [ -z "$tmpdir" ] || rm -rf "$tmpdir"
}
trap cleanup 0 1 2 3 15

if [ -n "$installer_url" ]; then
    installer=$tmpdir/install.sh
    download_installer "$installer_url" "$installer"
elif [ -f "$script_dir/install.sh" ]; then
    installer=$script_dir/install.sh
else
    installer=$tmpdir/install.sh
    download_installer \
        "https://raw.githubusercontent.com/$REPOSITORY/master/scripts/install.sh" \
        "$installer"
fi

install_dir=$tmpdir/bin
REWIRE_RELEASE=$release \
REWIRE_ASSET_BASE_URL=$asset_base_url \
REWIRE_DOWNLOAD_URL=$download_url \
REWIRE_CHECKSUM_URL=$checksum_url \
REWIRE_SHA256=$expected_sha256 \
    sh "$installer" --install-dir "$install_dir" --no-run --quiet

binary=$install_dir/rewire
[ -x "$binary" ] || fail "installer did not produce an executable rewire binary"

# Do not use exec: the wrapper must remove the temporary binary after Rewire exits.
if [ "$#" -eq 0 ]; then
    set -- configure
fi
if should_attach_terminal_input "$@"; then
    "$binary" "$@" < /dev/tty || status=$?
elif "$binary" "$@"; then
    status=0
else
    status=$?
fi
status=${status:-0}

cleanup
tmpdir=
trap - 0 1 2 3 15
exit "$status"
