#!/bin/sh

set -eu

REPOSITORY="CCH-HQ/rewire"
release=${REWIRE_RELEASE:-latest}
install_dir=${REWIRE_INSTALL_DIR:-}
asset_base_url=${REWIRE_ASSET_BASE_URL:-}
download_url=${REWIRE_DOWNLOAD_URL:-}
checksum_url=${REWIRE_CHECKSUM_URL:-}
expected_sha256=${REWIRE_SHA256:-}
run_after_install=1
quiet=0
asset_base_option_set=0
download_option_set=0
checksum_option_set=0
sha256_option_set=0

fail() {
    printf 'rewire installer: %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Install Rewire from GitHub Releases and optionally run it.

Usage:
  install.sh [INSTALLER OPTIONS] [--] [REWIRE ARGUMENTS...]

Installer options:
  --release <VERSION>       Release to install, for example 0.0.1 or v0.0.1
                            (default: latest)
  --install-dir <DIR>       Destination directory (default: $HOME/.local/bin)
  --asset-base-url <VALUE>  Release asset URL or local fixture/mirror directory
  --download-url <URL>      Exact platform archive URL or local file
  --checksum-url <URL>      Exact SHA256SUMS URL or local file
  --sha256 <DIGEST>         Expected archive SHA-256 instead of SHA256SUMS
  --no-run                  Install without starting Rewire
  --quiet                   Suppress installation status and PATH notices
  -h, --help                Print this help

With no Rewire arguments, the installer starts `rewire configure`. Otherwise,
all remaining arguments are passed to Rewire unchanged. Put `--` before Rewire
arguments when an argument could be confused with an installer option.
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
        --install-dir)
            require_value "$1" "$#"
            install_dir=$2
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
        --no-run)
            run_after_install=0
            shift
            ;;
        --quiet)
            quiet=1
            shift
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

if [ -z "$install_dir" ]; then
    [ -n "${HOME:-}" ] || fail "HOME is unset; pass --install-dir"
    install_dir=$HOME/.local/bin
fi
[ -n "$install_dir" ] || fail "--install-dir cannot be empty"

system=$(uname -s)
machine=$(uname -m)
case "$system:$machine" in
    Darwin:x86_64 | Darwin:amd64)
        target=x86_64-apple-darwin
        ;;
    Darwin:arm64 | Darwin:aarch64)
        target=aarch64-apple-darwin
        ;;
    Linux:x86_64 | Linux:amd64)
        target=x86_64-unknown-linux-gnu
        ;;
    Linux:arm64 | Linux:aarch64)
        target=aarch64-unknown-linux-gnu
        ;;
    *)
        fail "unsupported platform: $system $machine"
        ;;
esac

asset=rewire-$target.tar.gz
[ -z "$download_url" ] || [ -z "$asset_base_url" ] \
    || fail "--download-url conflicts with --asset-base-url"
[ -z "$expected_sha256" ] || [ -z "$checksum_url" ] \
    || fail "--sha256 conflicts with --checksum-url"
if [ -z "$download_url" ] && [ -z "$asset_base_url" ]; then
    case "$release" in
        latest)
            release_path=latest/download
            ;;
        v*)
            version=${release#v}
            case "$version" in
                '' | *[!0-9A-Za-z._-]*) fail "invalid release: $release" ;;
            esac
            release_path=download/v$version
            ;;
        *)
            case "$release" in
                '' | *[!0-9A-Za-z._-]*) fail "invalid release: $release" ;;
            esac
            release_path=download/v$release
            ;;
    esac
    asset_base_url=https://github.com/$REPOSITORY/releases/$release_path
fi

download_source() {
    source=$1
    destination=$2
    if [ -f "$source" ]; then
        cp "$source" "$destination" || fail "could not copy $source"
        return
    fi

    if command -v curl >/dev/null 2>&1; then
        case "$source" in
            https://*)
                curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
                    --retry 3 --retry-delay 1 --output "$destination" "$source" \
                    || fail "could not download $source"
                ;;
            *)
                curl --fail --location --silent --show-error --retry 3 --retry-delay 1 \
                    --output "$destination" "$source" || fail "could not download $source"
                ;;
        esac
    elif command -v wget >/dev/null 2>&1; then
        wget -q --tries=3 --output-document="$destination" "$source" \
            || fail "could not download $source"
    else
        fail "curl or wget is required"
    fi
}

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 "$1" | awk '{print $NF}'
    else
        fail "sha256sum, shasum, or openssl is required"
    fi
}

tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/rewire-install.XXXXXX") \
    || fail "could not create a temporary directory"
install_tmp=
cleanup() {
    [ -z "$install_tmp" ] || rm -f "$install_tmp"
    [ -z "$tmpdir" ] || rm -rf "$tmpdir"
}
trap cleanup 0 1 2 3 15

archive=$tmpdir/$asset
checksums=$tmpdir/SHA256SUMS
if [ -n "$download_url" ]; then
    archive_source=$download_url
else
    if [ -d "$asset_base_url" ]; then
        archive_source=$asset_base_url/$asset
    else
        archive_source=${asset_base_url%/}/$asset
    fi
fi
download_source "$archive_source" "$archive"

expected=$expected_sha256
if [ -z "$expected" ]; then
    if [ -z "$checksum_url" ]; then
        if [ -n "$download_url" ]; then
            checksum_url=${download_url%/*}/SHA256SUMS
        elif [ -d "$asset_base_url" ]; then
            checksum_url=$asset_base_url/SHA256SUMS
        else
            checksum_url=${asset_base_url%/}/SHA256SUMS
        fi
    fi
    download_source "$checksum_url" "$checksums"
    expected=$(awk -v asset="$asset" '$2 == asset || $2 == ("*" asset) { print $1; exit }' "$checksums")
fi
[ "${#expected}" -eq 64 ] || fail "expected SHA-256 is missing or invalid for $asset"
case "$expected" in
    *[!0-9A-Fa-f]*) fail "expected SHA-256 is missing or invalid for $asset" ;;
esac
actual=$(sha256_file "$archive")
[ -z "$expected" ] || expected=$(printf '%s' "$expected" | tr 'A-F' 'a-f')
actual=$(printf '%s' "$actual" | tr 'A-F' 'a-f')
[ "$actual" = "$expected" ] || fail "checksum mismatch for $asset"

extracted=$tmpdir/extracted
mkdir "$extracted"
binary_member=$(tar -tzf "$archive" | awk '
    $0 == "rewire" || $0 == "./rewire" { count += 1; member = $0 }
    END { if (count == 1) print member }
')
[ -n "$binary_member" ] || fail "$asset must contain exactly one rewire binary"
tar -xzf "$archive" -C "$extracted" "$binary_member" || fail "could not extract rewire"
if [ ! -f "$extracted/rewire" ] || [ -L "$extracted/rewire" ]; then
    fail "$asset does not contain a regular rewire binary"
fi

mkdir -p "$install_dir" || fail "could not create $install_dir"
install_tmp=$(mktemp "$install_dir/.rewire.XXXXXX") \
    || fail "could not create a temporary file in $install_dir"
cp "$extracted/rewire" "$install_tmp" || fail "could not stage rewire"
chmod 0755 "$install_tmp" || fail "could not make rewire executable"
destination=$install_dir/rewire
mv -f "$install_tmp" "$destination" || fail "could not install $destination"
install_tmp=

if [ "$quiet" -eq 0 ]; then
    printf 'Installed rewire to %s\n' "$destination"
    case ":${PATH:-}:" in
        *":$install_dir:"*) ;;
        *) printf 'Note: add %s to PATH to run rewire directly.\n' "$install_dir" >&2 ;;
    esac
fi

[ "$run_after_install" -eq 1 ] || exit 0

# Remove downloaded archives before replacing the shell with the installed binary.
rm -rf "$tmpdir"
tmpdir=
if [ "$#" -eq 0 ]; then
    set -- configure
fi
# `curl ... | sh` leaves the script pipe on stdin. Recover the controlling terminal for guided
# prompts, while preserving stdin for explicit token and automation modes.
if should_attach_terminal_input "$@"; then
    exec "$destination" "$@" < /dev/tty
fi
exec "$destination" "$@"
