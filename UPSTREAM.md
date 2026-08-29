# Upstream provenance

This source tree is derived from the `portable-pty` 0.9.0 crate.

- Upstream repository: <https://github.com/wezterm/wezterm>
- Upstream workspace path: `pty`
- Upstream source revision:
  `f8921727a11b9f8b073e8c24821d72fd41283500`
- Upstream license: MIT; see `LICENSE.md`

The compatibility delta is intentionally narrow:

- an object-safe guarded-spawn method on `SlavePty`;
- a Unix implementation whose prepared sysprims acquisition hook replaces
  portable-pty's internal `setsid` slot;
- an exact-child adapter owned by `sysprims_timeout::ContainmentGuard`;
- a pre-spawn parent recovery owner that retains the opaque exact child for
  bounded failure attempts and nonblocking error destruction;
- pre-spawn rejection for unsupported implementations, including Windows;
- lifecycle, real-PTY, allocator-lock, and compatibility tests; and
- mechanical current-Clippy fixes that do not change behavior.

The minimum sysprims contract is `v0.2.1`, commit
`0192fe424925f60536c5bdb93839eeb64175c857`. Compatibility is also checked
against the intended sysprims candidate before a companion release is cut.
