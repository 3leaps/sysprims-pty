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

stub_bin="$TMP_ROOT/stub-bin"
mkdir -p "$stub_bin"
cat > "$stub_bin/minisign" <<'EOF'
#!/bin/sh
exit 0
EOF
cat > "$stub_bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [ "$1" = "run" ] && [ "$2" = "list" ]; then
	case "${GH_STUB_MODE:-}" in
		workflow-success)
			printf '[{"databaseId":1,"headBranch":"%s","headSha":"%s","status":"completed","conclusion":"success","url":"https://example.invalid/run"}]\n' "$GH_STUB_TAG" "$GH_STUB_SHA"
			;;
		workflow-pending)
			printf '[{"databaseId":1,"headBranch":"%s","headSha":"%s","status":"in_progress","conclusion":"","url":"https://example.invalid/run"}]\n' "$GH_STUB_TAG" "$GH_STUB_SHA"
			;;
		*)
			printf '[]\n'
			;;
	esac
	exit 0
fi

if [ "$1" = "release" ] && [ "$2" = "view" ]; then
	if printf '%s\n' "$*" | grep -Fq ".isDraft"; then
		case "${GH_STUB_MODE:-}" in
			release-nondraft) printf 'false\n' ;;
			*) printf 'true\n' ;;
		esac
		exit 0
	fi
	if printf '%s\n' "$*" | grep -Fq ".assets[].name"; then
		case "${GH_STUB_MODE:-}" in
			release-unsigned-good)
				printf '%s\n' LICENSE-MIT "release-notes-${GH_STUB_TAG}.md" "sbom-${GH_STUB_VERSION}.cdx.json"
				;;
			release-unsigned-missing)
				printf '%s\n' LICENSE-MIT "release-notes-${GH_STUB_TAG}.md"
				;;
			release-signed-good)
				printf '%s\n' \
					LICENSE-MIT \
					"release-notes-${GH_STUB_TAG}.md" \
					"sbom-${GH_STUB_VERSION}.cdx.json" \
					SHA256SUMS \
					SHA256SUMS.minisig \
					SHA512SUMS \
					SHA512SUMS.minisig \
					sysprims-pty-minisign.pub
				;;
			release-signed-unexpected)
				printf '%s\n' \
					LICENSE-MIT \
					"release-notes-${GH_STUB_TAG}.md" \
					"sbom-${GH_STUB_VERSION}.cdx.json" \
					SHA256SUMS \
					SHA256SUMS.minisig \
					SHA512SUMS \
					SHA512SUMS.minisig \
					sysprims-pty-minisign.pub \
					stale-extra.zip
				;;
			*)
				printf '\n'
				;;
		esac
		exit 0
	fi
fi

if [ "$1" = "release" ] && { [ "$2" = "upload" ] || [ "$2" = "edit" ]; }; then
	printf '%s\n' "$*" >> "${GH_STUB_MUTATION_LOG:?}"
	exit 0
fi

echo "unexpected gh stub call: $*" >&2
exit 1
EOF
chmod +x "$stub_bin/minisign" "$stub_bin/gh"

expect_fail "SYSPRIMS_PTY_REQUIRE_TAG requires an annotated tag" \
	env SYSPRIMS_PTY_REQUIRE_TAG=1 "$copy_root/scripts/release-guard-tag-version.sh"

git -C "$copy_root" \
	-c user.name="release tooling test" \
	-c user.email="release-tooling-test@example.invalid" \
	tag -a "v${VERSION}" -m "v${VERSION}"
copy_sha="$(git -C "$copy_root" rev-parse "refs/tags/v${VERSION}^{commit}")"
expect_fail "tag workflow verifier requires completed success on exact tag commit" \
	env PATH="$stub_bin:$PATH" \
		GH_STUB_MODE=workflow-pending \
		GH_STUB_TAG="v${VERSION}" \
		GH_STUB_SHA="$copy_sha" \
		"$copy_root/scripts/verify-tag-release-workflow.sh" "v${VERSION}"
env PATH="$stub_bin:$PATH" \
	GH_STUB_MODE=workflow-success \
	GH_STUB_TAG="v${VERSION}" \
	GH_STUB_SHA="$copy_sha" \
	"$copy_root/scripts/verify-tag-release-workflow.sh" "v${VERSION}" >/dev/null
echo "[ok] tag workflow verifier accepts completed success on exact tag commit"

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

expect_fail "unsigned draft inventory rejects missing assets" \
	env PATH="$stub_bin:$PATH" \
		GH_STUB_MODE=release-unsigned-missing \
		GH_STUB_TAG="v${VERSION}" \
		GH_STUB_VERSION="$VERSION" \
		"$ROOT/scripts/verify-release-draft-assets.sh" "v${VERSION}" unsigned
env PATH="$stub_bin:$PATH" \
	GH_STUB_MODE=release-unsigned-good \
	GH_STUB_TAG="v${VERSION}" \
	GH_STUB_VERSION="$VERSION" \
	"$ROOT/scripts/verify-release-draft-assets.sh" "v${VERSION}" unsigned >/dev/null
echo "[ok] unsigned draft inventory accepts exact assets"

expect_fail "signed draft inventory rejects unexpected assets" \
	env PATH="$stub_bin:$PATH" \
		GH_STUB_MODE=release-signed-unexpected \
		GH_STUB_TAG="v${VERSION}" \
		GH_STUB_VERSION="$VERSION" \
		"$ROOT/scripts/verify-release-draft-assets.sh" "v${VERSION}" signed

pgp_dir="$TMP_ROOT/partial-pgp"
mkdir -p "$pgp_dir"
printf 'x  y\n' > "$pgp_dir/SHA256SUMS"
printf 'x  y\n' > "$pgp_dir/SHA512SUMS"
printf 'dummy\n' > "$pgp_dir/SHA256SUMS.minisig"
printf 'dummy\n' > "$pgp_dir/SHA512SUMS.minisig"
printf '%s\n' 'untrusted comment: fake public key for negative test' > "$pgp_dir/sysprims-pty-minisign.pub"
printf '%s\n' '-----BEGIN PGP PUBLIC KEY BLOCK-----' '-----END PGP PUBLIC KEY BLOCK-----' \
	> "$pgp_dir/sysprims-pty-release-signing-key.asc"
expect_fail "signature verification requires complete PGP signature set" \
	env PATH="$stub_bin:$PATH" "$ROOT/scripts/verify-signatures.sh" "$pgp_dir"

upload_dir="$TMP_ROOT/upload"
mkdir -p "$upload_dir"
for file in SHA256SUMS SHA256SUMS.minisig SHA512SUMS SHA512SUMS.minisig sysprims-pty-minisign.pub "release-notes-v${VERSION}.md"; do
	printf 'dummy\n' > "$upload_dir/$file"
done
mutation_log="$TMP_ROOT/gh-mutations.log"
expect_fail "upload refuses non-draft release before mutation" \
	env PATH="$stub_bin:$PATH" \
		GH_STUB_MODE=release-nondraft \
		GH_STUB_MUTATION_LOG="$mutation_log" \
		"$ROOT/scripts/upload-release-assets.sh" "v${VERSION}" "$upload_dir"
if [ -e "$mutation_log" ]; then
	echo "error: non-draft upload attempted release mutation" >&2
	exit 1
fi
echo "[ok] non-draft upload exits before release mutation"

echo "[ok] release tooling negative controls passed"
