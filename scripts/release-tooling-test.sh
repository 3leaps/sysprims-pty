#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
VERSION="$(tr -d '\r\n' < "$ROOT/VERSION")"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/sysprims-pty-release-tooling.XXXXXX")"
cleanup() {
	if [ -n "${TMP_ROOT:-}" ] && [ -d "$TMP_ROOT" ]; then
		find "$TMP_ROOT" -mindepth 1 -maxdepth 1 -exec rm -r {} +
		rmdir "$TMP_ROOT"
	fi
}
trap cleanup EXIT HUP INT TERM

expect_fail() {
	label="$1"
	shift
	if "$@" >/dev/null 2>&1; then
		echo "error: expected failure did not occur: $label" >&2
		exit 1
	fi
	echo "[ok] negative control failed as expected: $label"
}

copy_root="$TMP_ROOT/repo"
mkdir -p "$copy_root"
tar -C "$ROOT" \
	--exclude .git \
	--exclude target \
	--exclude dist \
	--exclude __pycache__ \
	-cf - . | tar -C "$copy_root" -xf -

git -C "$copy_root" init -q
git -C "$copy_root" add .
git -C "$copy_root" \
	-c user.name="release tooling test" \
	-c user.email="release-tooling-test@example.invalid" \
	commit -q -m "test fixture"

expect_fail "SYSPRIMS_PTY_REQUIRE_TAG requires an annotated tag" \
	env SYSPRIMS_PTY_REQUIRE_TAG=1 "$copy_root/scripts/release-guard-tag-version.sh"

asset_dir="$TMP_ROOT/assets"
mkdir -p "$asset_dir"
printf 'license\n' > "$asset_dir/LICENSE-MIT"
printf 'notes\n' > "$asset_dir/release-notes-v${VERSION}.md"
printf 'stale\n' > "$asset_dir/release-notes-v0.8.0.md"
printf '{}\n' > "$asset_dir/sbom-${VERSION}.cdx.json"
expect_fail "checksum generation rejects stale versioned assets" \
	"$ROOT/scripts/generate-checksums.sh" "v${VERSION}" "$asset_dir"

empty_dir="$TMP_ROOT/empty-keys"
mkdir -p "$empty_dir"
expect_fail "public key verification requires minisign public key" \
	"$ROOT/scripts/verify-public-keys.sh" "$empty_dir"

pgp_dir="$TMP_ROOT/partial-pgp"
mkdir -p "$pgp_dir"
printf 'x  y\n' > "$pgp_dir/SHA256SUMS"
printf 'x  y\n' > "$pgp_dir/SHA512SUMS"
printf 'dummy\n' > "$pgp_dir/SHA256SUMS.minisig"
printf 'dummy\n' > "$pgp_dir/SHA512SUMS.minisig"
printf '%s\n' 'untrusted comment: fake public key for negative test' > "$pgp_dir/sysprims-pty-minisign.pub"
printf '%s\n' '-----BEGIN PGP PUBLIC KEY BLOCK-----' '-----END PGP PUBLIC KEY BLOCK-----' \
	> "$pgp_dir/sysprims-pty-release-signing-key.asc"
stub_bin="$TMP_ROOT/stub-bin"
mkdir -p "$stub_bin"
cat > "$stub_bin/minisign" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$stub_bin/minisign"
expect_fail "signature verification requires complete PGP signature set" \
	env PATH="$stub_bin:$PATH" "$ROOT/scripts/verify-signatures.sh" "$pgp_dir"

echo "[ok] release tooling negative controls passed"
