#!/bin/sh

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/rewire-runner-test.XXXXXX")
cleanup() {
    rm -rf "$tmpdir"
}
trap cleanup 0 1 2 3 15

case "$(uname -s):$(uname -m)" in
    Darwin:x86_64 | Darwin:amd64) target=x86_64-apple-darwin ;;
    Darwin:arm64 | Darwin:aarch64) target=aarch64-apple-darwin ;;
    Linux:x86_64 | Linux:amd64) target=x86_64-unknown-linux-musl ;;
    Linux:arm64 | Linux:aarch64) target=aarch64-unknown-linux-musl ;;
    *) printf 'unsupported test platform\n' >&2; exit 1 ;;
esac

assets=$tmpdir/assets
package=$tmpdir/package
runner_tmp=$tmpdir/runner-tmp
persistent_install=$tmpdir/must-not-be-used
mkdir -p "$assets" "$package" "$runner_tmp"

# The fixture records both its temporary executable path and every received argument.
cat > "$package/rewire" <<'EOF'
#!/bin/sh
set -eu
: "${REWIRE_TEST_OUTPUT:?}"
{
    printf 'executable=%s\n' "$0"
    for argument in "$@"; do
        printf 'argument=%s\n' "$argument"
    done
} > "$REWIRE_TEST_OUTPUT"
if [ "${1:-}" = "--fixture-exit" ]; then
    exit "${2:-1}"
fi
if [ "${REWIRE_TEST_REQUIRE_TERMINAL:-0}" -eq 1 ]; then
    [ -t 0 ] || exit 91
    printf 'stdin=terminal\n' >> "$REWIRE_TEST_OUTPUT"
fi
if [ "${1:-}" = "--fixture-read-stdin" ]; then
    IFS= read -r input || true
    printf 'stdin=%s\n' "$input" >> "$REWIRE_TEST_OUTPUT"
fi
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
run_stdout=$tmpdir/run.stdout
REWIRE_INSTALL_DIR=$persistent_install \
REWIRE_TEST_OUTPUT=$output \
TMPDIR=$runner_tmp \
    sh "$root/scripts/run.sh" \
    --download-url "$assets/$asset" \
    --sha256 "$(printf '%s' "$digest" | tr 'a-f' 'A-F')" \
    -- \
    --baseurl "https://gateway.example/api path" \
    --client "claude,codex" \
    --dry-run > "$run_stdout"

cat > "$tmpdir/expected" <<'EOF'
argument=--baseurl
argument=https://gateway.example/api path
argument=--client
argument=claude,codex
argument=--dry-run
EOF
tail -n +2 "$output" > "$tmpdir/actual"
diff -u "$tmpdir/expected" "$tmpdir/actual"
[ ! -s "$run_stdout" ]
[ ! -e "$persistent_install" ]
output_line=$(sed -n '1p' "$output")
executable=${output_line#executable=}
[ ! -e "$executable" ]
if find "$runner_tmp" -mindepth 1 -maxdepth 1 -name 'rewire-run.*' | grep . >/dev/null; then
    printf 'temporary runner directory survived a successful run\n' >&2
    exit 1
fi

default_output=$tmpdir/default-arguments
REWIRE_TEST_OUTPUT=$default_output TMPDIR=$runner_tmp \
    sh "$root/scripts/run.sh" --asset-base-url "$assets"
printf 'argument=configure\n' > "$tmpdir/default-expected"
tail -n +2 "$default_output" > "$tmpdir/default-actual"
diff -u "$tmpdir/default-expected" "$tmpdir/default-actual"

# Exercise the run-only entrypoint with its source piped into a shell under a controlling PTY.
piped_output=$tmpdir/piped-arguments
REWIRE_TEST_OUTPUT=$piped_output \
REWIRE_TEST_REQUIRE_TERMINAL=1 \
TMPDIR=$runner_tmp \
    python3 "$root/scripts/tests/fixtures/pipe-with-tty.py" \
    sh -s -- --asset-base-url "$assets" \
    < "$root/scripts/run.sh" > "$tmpdir/piped.stdout"
printf 'argument=configure\nstdin=terminal\n' > "$tmpdir/piped-expected"
tail -n +2 "$piped_output" > "$tmpdir/piped-actual"
diff -u "$tmpdir/piped-expected" "$tmpdir/piped-actual"
piped_executable=$(sed -n 's/^executable=//p' "$piped_output")
[ ! -e "$piped_executable" ]

# Exercise inquire's actual event reader through the run-only source pipe. A Ctrl-C after the
# picker appears proves the first terminal read works without coupling this test to later prompts.
cargo build --locked --bin rewire
cp "$root/target/debug/rewire" "$package/rewire"
chmod 0755 "$package/rewire"
tar -C "$package" -czf "$assets/$asset" rewire
if command -v sha256sum >/dev/null 2>&1; then
    digest=$(sha256sum "$assets/$asset" | awk '{print $1}')
else
    digest=$(shasum -a 256 "$assets/$asset" | awk '{print $1}')
fi
printf '%s  %s\n' "$digest" "$asset" > "$assets/SHA256SUMS"

interactive_home=$tmpdir/interactive-home
mkdir -p "$interactive_home"
REWIRE_TEST_EXPECT='Choose one or more clients' \
REWIRE_TEST_INPUT="$(printf '\003')" \
TMPDIR=$runner_tmp \
    python3 "$root/scripts/tests/fixtures/pipe-with-tty.py" \
    sh -s -- --asset-base-url "$assets" -- \
    --baseurl https://gateway.example --token fixture-token \
    --home "$interactive_home" --dry-run --no-color \
    < "$root/scripts/run.sh" > "$tmpdir/interactive.stdout"
grep 'Choose one or more clients' "$tmpdir/interactive.stdout" >/dev/null
if grep 'Failed to initialize input reader' "$tmpdir/interactive.stdout" >/dev/null; then
    printf 'piped run-only entrypoint did not initialize the real terminal input reader\n' >&2
    exit 1
fi

# Restore the shell fixture for the remaining run-only boundary checks.
cat > "$package/rewire" <<'EOF'
#!/bin/sh
set -eu
: "${REWIRE_TEST_OUTPUT:?}"
{
    printf 'executable=%s\n' "$0"
    for argument in "$@"; do
        printf 'argument=%s\n' "$argument"
    done
} > "$REWIRE_TEST_OUTPUT"
if [ "${1:-}" = "--fixture-exit" ]; then
    exit "${2:-1}"
fi
if [ "${REWIRE_TEST_REQUIRE_TERMINAL:-0}" -eq 1 ]; then
    [ -t 0 ] || exit 91
    printf 'stdin=terminal\n' >> "$REWIRE_TEST_OUTPUT"
fi
if [ "${1:-}" = "--fixture-read-stdin" ]; then
    IFS= read -r input || true
    printf 'stdin=%s\n' "$input" >> "$REWIRE_TEST_OUTPUT"
fi
EOF
chmod 0755 "$package/rewire"
tar -C "$package" -czf "$assets/$asset" rewire
if command -v sha256sum >/dev/null 2>&1; then
    digest=$(sha256sum "$assets/$asset" | awk '{print $1}')
else
    digest=$(shasum -a 256 "$assets/$asset" | awk '{print $1}')
fi
printf '%s  %s\n' "$digest" "$asset" > "$assets/SHA256SUMS"

token_output=$tmpdir/token-stdin-arguments
printf 'fixture-token\n' | REWIRE_TEST_OUTPUT=$token_output TMPDIR=$runner_tmp \
    sh "$root/scripts/run.sh" --asset-base-url "$assets" -- \
    --fixture-read-stdin --token-stdin
grep '^stdin=fixture-token$' "$token_output" >/dev/null

non_interactive_output=$tmpdir/non-interactive-arguments
printf 'automation-input\n' | REWIRE_TEST_OUTPUT=$non_interactive_output TMPDIR=$runner_tmp \
    sh "$root/scripts/run.sh" --asset-base-url "$assets" -- \
    --fixture-read-stdin --non-interactive
grep '^stdin=automation-input$' "$non_interactive_output" >/dev/null

# A standalone copy can fetch its installer from an explicit local or remote source.
isolated=$tmpdir/isolated
mkdir "$isolated"
cp "$root/scripts/run.sh" "$isolated/run.sh"
downloaded_installer_output=$tmpdir/downloaded-installer-arguments
REWIRE_TEST_OUTPUT=$downloaded_installer_output TMPDIR=$runner_tmp \
    sh "$isolated/run.sh" \
    --installer-url "$root/scripts/install.sh" \
    --asset-base-url "$assets" -- --from-standalone
grep '^argument=--from-standalone$' "$downloaded_installer_output" >/dev/null

# Command-line sources replace conflicting environment defaults.
REWIRE_DOWNLOAD_URL=$tmpdir/missing-archive \
REWIRE_SHA256=$(printf '%064d' 0) \
REWIRE_TEST_OUTPUT=$tmpdir/precedence-arguments \
TMPDIR=$runner_tmp \
    sh "$root/scripts/run.sh" \
    --asset-base-url "$assets" \
    --checksum-url "$assets/SHA256SUMS" -- --precedence
grep '^argument=--precedence$' "$tmpdir/precedence-arguments" >/dev/null

if sh "$root/scripts/run.sh" --install-dir "$persistent_install" \
    > "$tmpdir/install-dir.stdout" 2> "$tmpdir/install-dir.stderr"
then
    printf 'run-only entrypoint accepted --install-dir\n' >&2
    exit 1
fi
grep 'reserved by the run-only entrypoint' "$tmpdir/install-dir.stderr" >/dev/null

printf '%064d  %s\n' 0 "$asset" > "$assets/SHA256SUMS"
checksum_output=$tmpdir/checksum-arguments
if REWIRE_TEST_OUTPUT=$checksum_output TMPDIR=$runner_tmp \
    sh "$root/scripts/run.sh" --asset-base-url "$assets" \
    > "$tmpdir/checksum.stdout" 2> "$tmpdir/checksum.stderr"
then
    printf 'checksum mismatch unexpectedly succeeded\n' >&2
    exit 1
fi
[ ! -e "$checksum_output" ]
grep 'checksum mismatch' "$tmpdir/checksum.stderr" >/dev/null

printf '%s  %s\n' "$digest" "$asset" > "$assets/SHA256SUMS"
set +e
REWIRE_TEST_OUTPUT=$tmpdir/exit-arguments TMPDIR=$runner_tmp \
    sh "$root/scripts/run.sh" --asset-base-url "$assets" -- --fixture-exit 23
exit_status=$?
set -e
[ "$exit_status" -eq 23 ] || {
    printf 'runner returned %s instead of fixture exit code 23\n' "$exit_status" >&2
    exit 1
}
exit_line=$(sed -n '1p' "$tmpdir/exit-arguments")
exit_executable=${exit_line#executable=}
[ ! -e "$exit_executable" ]

printf 'Unix run-only tests passed.\n'
