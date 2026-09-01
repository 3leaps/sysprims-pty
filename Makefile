.PHONY: help check check-candidate test-owned-pty-empty test-diabolical
.PHONY: version-check release-tooling-test release-guard-tag-version release-guard-tag-version-post
.PHONY: release-check release-preflight release-clean release-download release-notes
.PHONY: release-checksums release-sign release-export-keys release-verify-checksums
.PHONY: release-verify-signatures release-verify-keys release-verify release-upload release

VERSION := $(shell tr -d '\r\n' < VERSION 2>/dev/null || echo "dev")
SYSPRIMS_PTY_RELEASE_TAG ?= v$(VERSION)
DIST_RELEASE ?= dist/release
CARGO ?= cargo

help:
	@printf '%s\n' \
		'check                 Run formatting, lint, and host-safe tests' \
		'check-candidate       Check the exact sibling sysprims candidate' \
		'test-owned-pty-empty  Prove explicit-close and natural-exit PTY cleanup' \
		'test-diabolical       Run hostile containment scenes in disposable Docker' \
		'version-check         Validate VERSION, Cargo.toml, lockfile, and release docs' \
		'release-tooling-test  Run release-tooling negative controls' \
		'release-check         Run package and dry-run publish gates' \
		'release-preflight     Verify clean synced main and all pre-tag gates' \
		'release-download      Download draft release provenance assets' \
		'release-checksums     Copy notes and generate checksum manifests' \
		'release-sign          Sign checksum manifests locally' \
		'release-export-keys   Export public signing keys locally' \
		'release-verify        Verify checksums, signatures, and public-only keys' \
		'release-upload        Upload signed assets; does not undraft release'

check:
	for script in scripts/*.sh; do bash -n "$$script"; done
	python3 -m py_compile scripts/*.py
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	$(CARGO) test --all-targets --all-features

check-candidate:
	./scripts/run-candidate-check.sh

test-owned-pty-empty:
	./scripts/run-owned-pty-empty.sh

test-diabolical:
	./scripts/run-diabolical-docker.sh

version-check:
	python3 scripts/version-check.py

release-tooling-test:
	./scripts/release-tooling-test.sh

release-guard-tag-version:
	@SYSPRIMS_PTY_TAG_GUARD_MODE=$${MODE:-pre-tag} ./scripts/release-guard-tag-version.sh

release-guard-tag-version-post:
	@SYSPRIMS_PTY_TAG_GUARD_MODE=post-tag ./scripts/release-guard-tag-version.sh

release-check: version-check
	./scripts/check-package.sh

release-preflight:
	@echo "Running release preflight checks..."
	@if [ -n "$$(git status --porcelain 2>/dev/null)" ]; then \
		echo "error: working tree not clean" >&2; \
		git status --short >&2; \
		exit 1; \
	fi
	@git fetch origin >/dev/null 2>&1
	@if [ "$$(git rev-parse --abbrev-ref HEAD)" != "main" ]; then \
		echo "error: release preflight must run on main" >&2; \
		exit 1; \
	fi
	@if [ "$$(git rev-parse HEAD)" != "$$(git rev-parse origin/main)" ]; then \
		echo "error: local main is not synchronized with origin/main" >&2; \
		exit 1; \
	fi
	@$(MAKE) release-guard-tag-version MODE=pre-tag --silent
	@$(MAKE) check --silent
	@$(MAKE) release-check --silent
	@echo "[ok] release preflight passed"

release-clean:
	mkdir -p "$(DIST_RELEASE)"
	find "$(DIST_RELEASE)" -mindepth 1 -maxdepth 1 -exec rm -r {} +

release-download: release-clean release-guard-tag-version-post
	./scripts/download-release-assets.sh "$(SYSPRIMS_PTY_RELEASE_TAG)" "$(DIST_RELEASE)"

release-notes:
	mkdir -p "$(DIST_RELEASE)"
	cp "docs/releases/$(SYSPRIMS_PTY_RELEASE_TAG).md" "$(DIST_RELEASE)/release-notes-$(SYSPRIMS_PTY_RELEASE_TAG).md"
	@echo "[ok] copied release notes"

release-checksums: release-notes
	./scripts/generate-checksums.sh "$(SYSPRIMS_PTY_RELEASE_TAG)" "$(DIST_RELEASE)"

release-sign: release-guard-tag-version-post
	./scripts/sign-release-assets.sh "$(SYSPRIMS_PTY_RELEASE_TAG)" "$(DIST_RELEASE)"

release-export-keys:
	./scripts/export-release-keys.sh "$(DIST_RELEASE)"

release-verify-checksums:
	cd "$(DIST_RELEASE)" && shasum -a 256 -c SHA256SUMS && shasum -a 512 -c SHA512SUMS

release-verify-signatures:
	./scripts/verify-signatures.sh "$(DIST_RELEASE)"

release-verify-keys:
	./scripts/verify-public-keys.sh "$(DIST_RELEASE)"

release-verify: release-verify-checksums release-verify-signatures release-verify-keys
	@echo "[ok] release provenance verification passed"

release-upload: release-guard-tag-version-post release-verify
	./scripts/upload-release-assets.sh "$(SYSPRIMS_PTY_RELEASE_TAG)" "$(DIST_RELEASE)"

release: release-clean release-download release-checksums release-sign release-export-keys release-verify release-upload
	@echo "[ok] release assets uploaded; undraft requires a separate explicit cue"
