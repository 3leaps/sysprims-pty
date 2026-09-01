# Release checklist

This repository publishes a Rust library crate only. Do not add platform
archives, FFI bundles, Go modules, npm packages, or automated crates.io
publication.

## Prepare

- [ ] Confirm `VERSION`, `Cargo.toml`, `CHANGELOG.md`, and
      `docs/releases/vX.Y.Z.md` agree.
- [ ] Run `make release-preflight` from a clean `main` synchronized with
      `origin/main`.
- [ ] Open and merge the release-prep PR after review assent.

## Tag

```bash
VERSION=$(cat VERSION)
git switch main
git pull --ff-only origin main
make release-preflight
git tag -a "v${VERSION}" -m "v${VERSION}"
git push origin "v${VERSION}"
```

The tag workflow must create a draft GitHub release only. It must not sign
assets and must not publish to crates.io.

Before any crates.io upload, require the tag workflow to be green on the exact
tag and verify the unsigned draft release asset set:

```bash
VERSION=$(cat VERSION)
SYSPRIMS_PTY_REQUIRE_TAG=1 make release-guard-tag-version
make release-verify-tag-workflow
make release-verify-draft-assets
```

Expected unsigned draft assets:

- `LICENSE-MIT`
- `release-notes-vX.Y.Z.md`
- `sbom-X.Y.Z.cdx.json`

## First crates.io publication

Publication is irreversible and requires a separate explicit maintainer cue.
Use `CARGO_REGISTRY_TOKEN` or an equivalent Cargo credential source with the
minimum required crate scope. Do not write token material or owner identities
to tracked files, release notes, PR text, or planning files.

```bash
VERSION=$(cat VERSION)
git checkout --detach "v${VERSION}"
SYSPRIMS_PTY_REQUIRE_TAG=1 make release-guard-tag-version
cargo info --registry crates-io sysprims-pty
cargo publish --dry-run --locked
cargo publish --locked
cargo info --registry crates-io "sysprims-pty@${VERSION}"
```

Stop if the crate name resolves to an unexpected owner or if the dry-run
tarball differs from the reviewed package.

## Sign and upload release provenance

After the draft GitHub release exists and the crates.io publish cue has been
handled:

Required local signing environment:

- `SYSPRIMS_PTY_RELEASE_TAG`: release tag with the leading `v`, for example
  `v0.9.1`.
- `SYSPRIMS_PTY_MINISIGN_KEY`: minisign secret key path.
- `SYSPRIMS_PTY_MINISIGN_PUB`: minisign public key path. If omitted, the
  tooling derives it from `SYSPRIMS_PTY_MINISIGN_KEY` by replacing `.key` with
  `.pub`.

Optional PGP signing is enabled by key id:

- `SYSPRIMS_PTY_PGP_KEY_ID`: PGP signing key id.
- `SYSPRIMS_PTY_GPG_HOMEDIR`: optional alternate GPG home, if needed.

```bash
export SYSPRIMS_PTY_RELEASE_TAG=v$(cat VERSION)
make release-download
make release-checksums
make release-sign
make release-export-keys
make release-verify
make release-upload
```

After upload, the release must still be a draft and must contain the signed
asset inventory:

```bash
make release-verify-draft-assets MODE=signed
```

Before publishing the GitHub release, run the registry-only consumer smoke from
a clean checkout after crates.io shows the package:

```bash
cargo info --registry crates-io "sysprims-pty@$(cat VERSION)"
make release-smoke-consumer
make release-download-signed
make release-verify
```

Publishing the GitHub release is a separate explicit maintainer cue after the
checks above:

```bash
gh release edit "v$(cat VERSION)" --draft=false
```

## Post-release verification

- [ ] `vX.Y.Z` is annotated and peels to the reviewed commit.
- [ ] GitHub release is public and contains notes, license, SBOM, checksums,
      signatures, and public keys.
- [ ] `make release-download-signed` then `make release-verify` pass from
      freshly downloaded signed assets.
- [ ] `cargo info --registry crates-io sysprims-pty@X.Y.Z` resolves.
- [ ] docs.rs recognizes the published version.
- [ ] `make release-smoke-consumer` passes with a registry-only temporary
      consumer that builds with:

  ```toml
  [dependencies]
  sysprims-pty = "=X.Y.Z"
  ```

  and imports `portable_pty`.
