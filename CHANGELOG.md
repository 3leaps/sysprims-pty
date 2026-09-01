# Changelog

All notable changes to `sysprims-pty` are documented here.

## 0.9.1 - 2026-09-01

- First public release for the crates.io publication.
- Adds Rustfmt to the tag release workflow package gate so `make check` is
  fully provisioned on Rust 1.88.0.
- Leaves `v0.9.0` as a non-release annotated tag with no GitHub release and no
  crates.io publication.

## 0.9.0 - 2026-09-01

- Initial release-preparation tag. No GitHub release or crates.io publication.
- Configures the `sysprims-pty` crate while retaining the `portable_pty`
  library target for migration compatibility.
- Uses registry-resolvable `sysprims-session` and `sysprims-timeout` 0.2.3
  dependencies.
- Adds release/version/package gates and a draft-only GitHub release workflow.
