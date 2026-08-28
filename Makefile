.PHONY: check test-diabolical

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test --all-targets --all-features

test-diabolical:
	./scripts/run-diabolical-docker.sh
