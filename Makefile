.PHONY: help check test-owned-pty-empty test-diabolical

help:
	@printf '%s\n' \
		'check                 Run formatting, lint, and host-safe tests' \
		'test-owned-pty-empty  Prove explicit-close and natural-exit PTY cleanup' \
		'test-diabolical       Run hostile containment scenes in disposable Docker'

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test --all-targets --all-features

test-owned-pty-empty:
	./scripts/run-owned-pty-empty.sh

test-diabolical:
	./scripts/run-diabolical-docker.sh
