# sysprims-pty

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![Rust: 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)

Cross-platform **owned PTY sessions** with race-free process containment,
built on [sysprims](https://github.com/3leaps/sysprims).

Use this crate when you need a real native PTY or ConPTY, opaque byte I/O and
resize, and an honest final outcome for the exact child and its descendants
when the tab, test, or agent closes.

It is derived from [`portable-pty`](https://crates.io/crates/portable-pty)
0.9.0, so familiar command / pair / master / slave / I/O / resize types remain
available as a **migration lane**. Compatibility is the on-ramp, not the
product: this crate does not promise identity with upstream `portable-pty`,
and it adds what portable-pty leaves to you — race-free contained spawn and
an owned, verifiable process lifecycle.

The package name is `sysprims-pty`. The library target is `portable_pty`,
so existing `portable_pty` imports can keep working after a package rename.
The first public version coordinate is `0.9.1`.

## What you can build

One launch, one owned session, one honest final outcome. Use it for an agent
console, IDE terminal panel, interactive test harness, REPL supervisor, or
task runner that:

1. calls `SlavePty::spawn_contained_command` — unsupported implementations
   reject before spawning; there is no separate support-query API in this
   tree,
2. on success, holds an owned `ContainmentGuard<ContainedPtyChild>` from
   that single spawn-time transaction,
3. talks to the PTY with opaque byte I/O and resize,
4. closes explicitly or lets the leader complete naturally,
5. reads containment, completion, and reap evidence with no process-group,
   Job, or wait glue of your own.

On Unix, `SlavePty::spawn_contained_command` installs the prepared sysprims
acquisition hook in the PTY-owned spawn and validates its sealed same-spawn
receipt. On Windows, the ConPTY adapter creates the child suspended exactly
once, assigns and verifies that exact process in a prepared non-breakaway Job,
transfers sole process/Job authority to the guard, and resumes the primary
thread exactly once. Both paths return an owned
`ContainmentGuard<ContainedPtyChild>`.

## What this is not

- a terminal emulator (no VT parsing, scrollback, or rendering)
- a multiplexer or collaboration layer
- a shell policy / authorization product
- a general process supervisor or generic spawner
- a drop-in replacement that matches every `portable-pty` API and bug
- an OS non-escape sandbox

Generic process identity, Job/group evidence, and receipts live in
`sysprims`. This crate composes those primitives around the PTY spawn seam.

## Containment semantics

- `TreeKillReliability::Guaranteed` means race-free session/group
  acquisition and group-signaling eligibility.
- The guard exclusively owns child observation and reap through a terminal
  lifecycle transition.
- Completion evidence is reported independently as `Empty`, `Survivors`,
  or `Unknown`.
- Boundary strength is independent too: Unix reports `cooperative_group`; the
  pre-execution Windows Job path reports `kernel_enforced_job`.
- A cooperative Unix descendant can still leave its acquired group.
  Guaranteed acquisition is not an OS-enforced non-escape guarantee.
- Unsupported PTY implementations reject the guarded API before spawning.

## Owned real-PTY examples

The integration tests contain two small, executable public-API examples. Each
opens a real controlling PTY, starts at least one descendant, and proves
`Guaranteed` acquisition, supported `Empty` completion, and exact leader reap:

1. [`owned_empty_explicit_close_with_descendant`](tests/contained_spawn.rs)
   calls `ContainmentGuard::terminate`.
2. [`owned_empty_natural_leader_exit_with_descendant`](tests/contained_spawn.rs)
   polls `ContainmentGuard::try_complete`, retrying only `Ok(None)` while the
   leader is running. Any error fails the scene.

```console
make test-owned-pty-empty
```

The runner uses the sibling `../sysprims` checkout by default, verifies the
reviewed sysprims revision, then applies it as a local source override in an
isolated copy. Set `SYSPRIMS_ROOT` to use another checkout. The package
manifest retains registry-resolvable released dependencies; this target proves
a separately reviewed candidate without changing the public package edges.

The target is intentionally separate from ordinary CI because its evidence
depends on real Unix PTY and process-group behavior. `make help` lists the
other local verification targets.

Application code should construct commands with `CommandBuilder`. The test
fixture invokes `/bin/sh` only to create deterministic descendant processes.

## Provenance

This tree is derived from the `pty` crate in the wezterm repository. See
[UPSTREAM.md](UPSTREAM.md) and [LICENSE-MIT](LICENSE-MIT).

## License

MIT. See [LICENSE-MIT](LICENSE-MIT).

Upstream `portable-pty` is MIT (Copyright 2018 Wez Furlong). 3 Leaps
modifications are MIT as well. This crate is **not** dual-licensed
MIT/Apache-2.0 like `sysprims`; the upstream grant is MIT-only.

This project follows the [3 Leaps OSS policies](https://github.com/3leaps/oss-policies).
