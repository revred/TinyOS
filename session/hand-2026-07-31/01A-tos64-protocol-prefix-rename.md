# Handover 01A (2026-07-31) — Owner-Ordered: the `TINYOS-*` Envelope Prefix Is Now `TOS64-*`

The owner's direction, verbatim in effect: *TINYOS as all caps looks very unreadable —
it should be TOS64.* Applied to every all-caps protocol envelope prefix, live,
repo-wide, in one commit.

## What changed

Every wire/serial envelope this system emits or parses renamed `TINYOS-*` →
`TOS64-*`, **payload format and version suffixes unchanged** — `TOS64-RESULT/1` is
byte-for-byte the old `TINYOS-RESULT/1` under the new prefix, so no version bump:

`TOS64-BOOT/1` · `TOS64-RESULT/1` · `TOS64-MEAS/2` (and the `/1`, `/3` mentions in
parser rejection tests) · `TOS64-SPOOR/1` · `TOS64-BOUND/1` · `TOS64-FAULT/1` ·
`TOS64-PANIC/1` · `TOS64-UNROUTED/1`

Surface: 20 `.rs` files (kernel, hal, hal-arm64, hal-x86_64, exec, shell, pi5-image,
xtask emitters *and* parsers *and* their byte-pinning cross-tests), 2 `Cargo.toml`
descriptions, `.github/workflows/ci.yml`'s serial-grep gates, and every **living**
governing document (assurance README and contract TSVs, Feature/Story/Test documents,
dashboards, `docs/`, `work/`).

## The line drawn (the 09A `MEAS/1→/2` precedent)

**Dated evidence is immutable.** `goals/reports/*` (including capture assets and
measure logs) and `session/*` still say `TINYOS-*`, because they are records of what
those runs actually emitted. The parity golden transcript and the timing baselines
contain no envelope strings, so nothing was regenerated.

## Proof it holds together

- `cargo test --workspace`: **796 tests, 0 failures** — the emitter/parser byte
  cross-pins renamed in lockstep or these would have caught it.
- **Real Tier 0 boots**: default boot and `actuation` fixtures green under local
  QEMU; `check-shell-parity` green with all three signals, the third being the
  renamed trailer parsed from a live guest: `TOS64-SPOOR/1 len=1 denials=1`.
- `check-spine-files`, `check-assurance-spine`, `cargo fmt --all --check` green.
- **The Pi 5 image was rebuilt** (its banner bytes changed):
  `kernel8.img` now **82,908 bytes, sha256 `7e9d6b87…3eea8`**, restaged with
  `config.txt` under `os/target/pi5/`. Any board session must use this image; a
  capture from the old image would show `TINYOS-*` and fail the new parser.

## Note for readers of older documents

A `TINYOS-*` string in a Report or handover dated ≤ 2026-07-30 is the same envelope
under its old name. The board-session checklist in `hand-2026-07-30/09A` remains
correct except the expected verdict line, which now reads
`TOS64-RESULT/1 fixture=boot ok=true`.
