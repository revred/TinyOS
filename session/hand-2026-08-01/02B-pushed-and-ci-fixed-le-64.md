# Handover 02B — The Day's Work Pushed, and the CI Red Nobody Had Read (`LE-64`)

**Owner-ordered 2026-08-01: commit and push.** The 01B planning session's artifacts were found
uncommitted in the working tree and landed as their own commit (read in full first, per
[`CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) rule 3); `main` was then pushed —
16 commits, `8f8dbc0..fc04161` — after a fetch confirmed origin had not moved.

## What the push surfaced

**CI on `main` had been red since 2026-07-30, across two pushes, and no one had read it.** The
fixing session watched run `30716559202` to completion, compared its failure signature against the
two prior red runs (`30540669984`, `30531476802`), and found all three identical — the red predated
today's 16 commits entirely.

**Root cause (`LE-64`, raised and closed in this session):** the shell crate includes
`spoor_policy.rs` via `#[path = "../spoor_policy.rs"]` from inside two *inline* modules (`aci` in
[`fixture_batch_main.rs`](../../os/src/shell/src/fixture_batch_main.rs), `spoor_policy_host` in
[`lib.rs`](../../os/src/shell/src/lib.rs)). An inline module implies a directory (`src/aci/`,
`src/spoor_policy_host/`) that exists on no disk, so the resolved path traverses a phantom
component. **Windows normalises `aci/..` lexically and opens the file; Linux resolves
component-by-component and fails with ENOENT** — so rustfmt and every shell build broke on the CI
runner while every local Windows gate stayed green, for two days.

**Fix:** `#[path = "."]` on the two inline modules, so the nested `#[path = "spoor_policy.rs"]`
resolves through `src/` itself — no phantom component on either OS. No behaviour change; the
`use super::` seam `spoor_policy.rs` documents is untouched. Validated locally by
`cargo test -p shell --lib` (33 green), `cargo fmt -p shell -- --check`, and
`check-shell-parity` (transcript matches golden, in-guest assertions pass, spoor journal
corroborates); validated for real by the CI run on the fixing commit, which is the only resolver
that was ever broken.

## The finding worth keeping

The defect was two lines. The process hole was bigger: **a push whose CI run is not watched is a
gate that does not exist.** Two sessions pushed onto red without reading it, and the red carried no
register row — it lived nowhere but in a GitHub tab nobody had open. `LE-64` records both halves.

## Postscript: the red was two failures deep

The `LE-64` fix's own run (`30716801991`) cleared rustfmt and every shell build — and then failed
on the **next** failure the rustfmt step had been masking since 07-30: a
`clippy::deref_addrof` error on `installed_ring0_stack_top()` in
[`gdt.rs`](../../os/src/hal-x86_64/src/gdt.rs), whose sibling function directly above already
carries the sanctioned `#[allow]` for the same `static mut` readback pattern. It was never seen
locally because the function is `cfg(not(windows))` — local clippy compiles it out, the same
Windows-blindness class as `LE-64` itself. Fixed by mirroring **both** CI clippy jobs locally
before pushing (workspace clippy cross-targeted to `x86_64-unknown-linux-gnu`, and the AArch64
`-Zbuild-std` job), both exit 0. The follow-up commit's run is **green — the first green CI on
`main` since 2026-07-27.** The practical rule this adds to `LE-64`'s: on a Windows dev host,
`cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu` is the honest local
mirror of CI's lint gate; host-target clippy is structurally blind to every `cfg(not(windows))`
line in this repository.

## State after this session

- `main` pushed and, once the fixing commit's run is green, CI-green for the first time since
  2026-07-30. The `_soak-p0-03-01.log` one-line append remains deliberately uncommitted — it
  belongs to the soak run.
- The TinyTile review gate ([`01C`](01C-next-steps-after-tinytile-planning.md) Step 1) is
  unchanged and still open; nothing in this session touched the planning artifacts.
