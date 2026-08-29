# Upstream provenance

This source tree is derived from the `portable-pty` 0.9.0 crate.

- Upstream repository: <https://github.com/wezterm/wezterm>
- Published crate SHA-256:
  `b4a596a2b3d2752d94f51fac2d4a96737b8705dddd311a32b9af47211f08671e`
- Upstream workspace path: `pty`
- Upstream source revision:
  `f8921727a11b9f8b073e8c24821d72fd41283500`
- Upstream license: MIT; see `LICENSE-MIT`

The compatibility delta is intentionally narrow:

- an object-safe guarded-spawn method on `SlavePty`;
- a Unix implementation whose prepared sysprims acquisition hook replaces
  portable-pty's internal `setsid` slot;
- a Windows ConPTY transaction that creates one child suspended, assigns and
  verifies that exact process in a non-breakaway Job, then resumes once;
- an exact-child adapter owned by `sysprims_timeout::ContainmentGuard`;
- a pre-spawn parent recovery owner that retains the opaque exact child for
  bounded failure attempts and nonblocking error destruction;
- pre-spawn rejection for unsupported PTY implementations;
- lifecycle, real-PTY, allocator-lock, and compatibility tests; and
- mechanical current-Clippy fixes that do not change behavior.

The minimum sysprims contract is `v0.2.2`, commit
`7e5cc03847029dbd316d9f8c0887997bf64a247c`. Compatibility is also checked
against that exact sysprims revision before a companion release is cut.
