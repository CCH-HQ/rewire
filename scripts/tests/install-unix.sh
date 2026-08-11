#!/bin/sh

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/rewire-installer-test.XXXXXX")
cleanup() {
    rm -rf "$tmpdir"
}
trap cleanup 0 1 2 3 15

case "$(uname -s):$(uname -m)" in
    Darwin:x86_64 | Darwin:amd64) target=x86_64-apple-darwin ;;
    Darwin:arm64 | Darwin:aarch64) target=aarch64-apple-darwin ;;
    Linux:x86_64 | Linux:amd64) target=x86_64-unknown-linux-gnu ;;
    Linux:arm64 | Linux:aarch64) target=aarch64-unknown-linux-gnu ;;
    *) printf 'unsupported test platform\n' >&2; exit 1 ;;
esac

assets=$tmpdir/assets
package=$tmpdir/package
install_dir=$tmpdir/install
mkdir -p "$assets" "$package"

cat > "$package/rewire" <<'EOF'
#!/bin/sh
set -eu
: "${REWIRE_TEST_OUTPUT:?}"
: > "$REWIRE_TEST_OUTPUT"
for argument in "$@"; do
    printf '%s\n' "$argument" >> "$REWIRE_TEST_OUTPUT"
done
EOF
chmod 0755 "$package/rewire"

asset=rewire-$target.tar.gz
tar -C "$package" -czf "$assets/$asset" rewire
if command -v sha256sum >/dev/null 2>&1; then
    digest=$(sha256sum "$assets/$asset" | awk '{print $1}')
else
    digest=$(shasum -a 256 "$assets/$asset" | awk '{print $1}')
fi
printf '%s  %s\n' "$digest" "$asset" > "$assets/SHA256SUMS"

output=$tmpdir/arguments
REWIRE_RELEASE='../ignored-for-direct-download' \
REWIRE_TEST_OUTPUT=$output sh "$root/scripts/install.sh" \
    --download-url "$assets/$asset" \
    --sha256 "$(printf '%s' "$digest" | tr 'a-f' 'A-F')" \
    --install-dir "$install_dir" \
    -- \
    --baseurl "https://gateway.example/api path" \
    --client "claude,codex" \
    --dry-run

cat > "$tmpdir/expected" <<'EOF'
--baseurl
https://gateway.example/api path
--client
claude,codex
--dry-run
EOF
diff -u "$tmpdir/expected" "$output"
[ -x "$install_dir/rewire" ]

default_output=$tmpdir/default-arguments
REWIRE_TEST_OUTPUT=$default_output sh "$root/scripts/install.sh" \
    --asset-base-url "$assets" \
    --install-dir "$install_dir"
printf 'configure\n' > "$tmpdir/default-expected"
diff -u "$tmpdir/default-expected" "$default_output"

no_run_output=$tmpdir/no-run-arguments
REWIRE_TEST_OUTPUT=$no_run_output sh "$root/scripts/install.sh" \
    --asset-base-url "$assets" \
    --install-dir "$install_dir" \
    --no-run
[ ! -e "$no_run_output" ]

printf 'existing installation must survive\n' > "$install_dir/rewire"
before=$(if command -v sha256sum >/dev/null 2>&1; then sha256sum "$install_dir/rewire" | awk '{print $1}'; else shasum -a 256 "$install_dir/rewire" | awk '{print $1}'; fi)
printf '%064d  %s\n' 0 "$asset" > "$assets/SHA256SUMS"
if sh "$root/scripts/install.sh" \
    --asset-base-url "$assets" \
    --install-dir "$install_dir" \
    --no-run > "$tmpdir/checksum.stdout" 2> "$tmpdir/checksum.stderr"
then
    printf 'checksum mismatch unexpectedly succeeded\n' >&2
    exit 1
fi
after=$(if command -v sha256sum >/dev/null 2>&1; then sha256sum "$install_dir/rewire" | awk '{print $1}'; else shasum -a 256 "$install_dir/rewire" | awk '{print $1}'; fi)
[ "$before" = "$after" ]
grep 'checksum mismatch' "$tmpdir/checksum.stderr" >/dev/null

if sh "$root/scripts/install.sh" --release '../invalid' --no-run \
    > "$tmpdir/release.stdout" 2> "$tmpdir/release.stderr"
then
    printf 'invalid release unexpectedly succeeded\n' >&2
    exit 1
fi
grep 'invalid release' "$tmpdir/release.stderr" >/dev/null

# Explicit command-line sources override conflicting source values from the environment.
printf '%s  %s\n' "$digest" "$asset" > "$assets/SHA256SUMS"
REWIRE_DOWNLOAD_URL=$tmpdir/missing \
REWIRE_SHA256=$(printf '%064d' 0) \
REWIRE_TEST_OUTPUT=$tmpdir/precedence-arguments \
    sh "$root/scripts/install.sh" \
    --asset-base-url "$assets" \
    --checksum-url "$assets/SHA256SUMS" \
    --install-dir "$install_dir" \
    --no-run

if sh "$root/scripts/install.sh" \
    --download-url "$assets/$asset" \
    --asset-base-url "$assets" \
    --no-run > "$tmpdir/conflict.stdout" 2> "$tmpdir/conflict.stderr"
then
    printf 'conflicting download sources unexpectedly succeeded\n' >&2
    exit 1
fi
grep 'conflicts with' "$tmpdir/conflict.stderr" >/dev/null

printf 'Unix installer tests passed.\n'
