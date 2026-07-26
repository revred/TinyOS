# ADR 0002 — No MSVC/Visual Studio Dependency for Windows Host Builds

Status: **Accepted**
Date: 2026-07-26
Introduced in: [`session/hand-2026-07-26/`](../../session/hand-2026-07-26/) (Phase 0 walking skeleton implementation)

## Context

`xtask` (and any future `std` host tooling) targets `x86_64-pc-windows-msvc` by default on a Windows contributor machine. That target's usual linker is MSVC's `link.exe`, which requires a full Visual Studio or Visual Studio Build Tools install — a multi-gigabyte, elevation-requiring, Windows-only piece of developer-machine state that [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#tooling)'s "no developer-machine-specific state is allowed to leak into a build" principle argues against requiring. Separately, on this machine an attempted VS 2022 Build Tools install failed (installer exit code 1602) — Microsoft's own installer, not something this project controls the reliability of.

The sibling project `Sharc.Blue` (`Sharc.Bluekind/.cargo/config.toml`) already solved this exact problem: pin `[target.x86_64-pc-windows-msvc] linker = "rust-lld"` (LLVM's linker, bundled with `rustup` — no separate install) and supply the Windows SDK/CRT import libraries (`kernel32.lib`, `ntdll.lib`, etc.) via `LIB`, sourced from the `cargo-xwin` splat cache rather than a Visual Studio install.

## Decision

TinyOS's `os/.cargo/config.toml` adopts the same pattern: `rust-lld` as the MSVC-target linker, `LIB` pointed at a `cargo-xwin`-populated import-library cache. No crate in this workspace is built with a dependency on Visual Studio or Windows Build Tools being installed.

This is scoped to **host tooling only** (`xtask`, and any future `deploy-client`/`bridge-host`-style `std` crate). `kernel`/`hal`/`hal-x86_64` already avoid this entirely — they build against the custom `os/targets/x86_64-tinyos.json` bare-metal target with `linker-flavor: "ld.lld"` (see [ADR 0001](0001-nightly-toolchain-for-build-std.md)), which never touches `link.exe` or the Windows SDK.

CI does not inherit this concern at all: [`ci.yml`](../../.github/workflows/ci.yml) runs on `ubuntu-latest`, where `xtask` links against the host Linux triple with no MSVC-equivalent dependency in the first place.

## Consequences

- A Windows contributor needs `cargo-xwin`'s splat cache populated (`cargo xwin` run once) before `xtask` builds locally, instead of a Visual Studio install — smaller, no elevation, no GUI installer.
- The current `LIB` value is a **known gap**, carried over from `Sharc.Blue`'s own unresolved "F40" issue: cargo's `[env]` table is process-global, not target-gated, and the path is this machine's absolute, username-bearing path — not reproducible for another contributor as committed. This is an explicit, tracked limitation, not an oversight; the proper fix (a host-side tool that discovers the xwin cache path dynamically, e.g. via `Sharc.Atomics`'s `BoundedProcess` pattern) is deferred, matching the sibling project's own deferral, and should be revisited before this config is relied on by a second Windows contributor.
- macOS and Linux hosts are unaffected — they link via their native toolchain (`cc`/`clang`) with no equivalent override needed.
