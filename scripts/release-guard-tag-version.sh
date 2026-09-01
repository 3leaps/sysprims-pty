#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${SYSPRIMS_PTY_REPO_ROOT:-$(git -C "$SCRIPT_DIR/.." rev-parse --show-toplevel)}"
MODE="${SYSPRIMS_PTY_TAG_GUARD_MODE:-${MODE:-pre-tag}}"
if [ "${SYSPRIMS_PTY_REQUIRE_TAG:-0}" = "1" ]; then
	MODE="post-tag"
fi

cd "$PROJECT_ROOT"

fail() {
	echo "error: $*" >&2
	exit 1
}

python3 scripts/version-check.py

VERSION="$(tr -d '\r\n' < VERSION)"
EXPECTED_TAG="v${VERSION}"
INTENDED_TAG="${SYSPRIMS_PTY_RELEASE_TAG:-$EXPECTED_TAG}"

[ "$INTENDED_TAG" = "$EXPECTED_TAG" ] ||
	fail "intended release tag ${INTENDED_TAG} does not equal ${EXPECTED_TAG}"

case "$MODE" in
pre-tag)
	if [ -n "$(git status --porcelain)" ]; then
		git status --short >&2
		fail "pre-tag guard requires a clean working tree"
	fi
	echo "[ok] pre-tag guard: clean coherent pack intends ${EXPECTED_TAG}"
	;;
post-tag)
	REF="refs/tags/${EXPECTED_TAG}"
	git show-ref --verify --quiet "$REF" ||
		fail "exact tag ${EXPECTED_TAG} does not exist"
	[ "$(git cat-file -t "$REF")" = "tag" ] ||
		fail "tag ${EXPECTED_TAG} must be annotated"
	HEAD_COMMIT="$(git rev-parse 'HEAD^{commit}')"
	TAG_COMMIT="$(git rev-parse "${REF}^{commit}")"
	[ "$TAG_COMMIT" = "$HEAD_COMMIT" ] ||
		fail "tag ${EXPECTED_TAG} peels to ${TAG_COMMIT}, not HEAD ${HEAD_COMMIT}"
	echo "[ok] post-tag guard: ${EXPECTED_TAG} is annotated and points at HEAD ${HEAD_COMMIT}"
	;;
*)
	fail "unknown tag guard mode ${MODE}; expected pre-tag or post-tag"
	;;
esac
