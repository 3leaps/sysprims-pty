#!/usr/bin/env bash
set -euo pipefail

TAG=${1:-${SYSPRIMS_PTY_RELEASE_TAG:-}}
if [ -z "$TAG" ]; then
	echo "usage: verify-tag-release-workflow.sh <tag>" >&2
	exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${SYSPRIMS_PTY_REPO_ROOT:-$(git -C "$SCRIPT_DIR/.." rev-parse --show-toplevel)}"
cd "$PROJECT_ROOT"

VERSION="${TAG#v}"
if [ "$TAG" != "v${VERSION}" ]; then
	echo "error: release tag must start with v: ${TAG}" >&2
	exit 1
fi

expected="$(git rev-parse "refs/tags/${TAG}^{commit}")"
runs="$(gh run list \
	--workflow Release \
	--event push \
	--json databaseId,headBranch,headSha,status,conclusion,url \
	--limit 50)"

RUNS_JSON="$runs" EXPECTED_SHA="$expected" TAG="$TAG" python3 - <<'PY'
import json
import os
import sys

runs = json.loads(os.environ["RUNS_JSON"])
expected = os.environ["EXPECTED_SHA"]
tag = os.environ["TAG"]

matches = [
    run
    for run in runs
    if run.get("headBranch") == tag and run.get("headSha") == expected
]
if not matches:
    print(
        f"error: no Release workflow run found for {tag} at {expected}",
        file=sys.stderr,
    )
    raise SystemExit(1)

successful = [
    run
    for run in matches
    if run.get("status") == "completed" and run.get("conclusion") == "success"
]
if not successful:
    print(
        f"error: Release workflow for {tag} at {expected} is not completed/success",
        file=sys.stderr,
    )
    for run in matches:
        print(
            f"  run {run.get('databaseId')} status={run.get('status')} "
            f"conclusion={run.get('conclusion')} url={run.get('url')}",
            file=sys.stderr,
        )
    raise SystemExit(1)

run = successful[0]
print(
    f"[ok] Release workflow run {run.get('databaseId')} succeeded for {tag} at {expected}: {run.get('url')}"
)
PY
