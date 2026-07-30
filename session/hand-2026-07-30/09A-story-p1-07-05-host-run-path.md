# Handover 09A — 08A Executed, Part One: `STORY-P1-07-05`, the Pi 5 Host-Side Run Path

Follows: [`08A-hardware-evidence-sprint-mandate.md`](08A-hardware-evidence-sprint-mandate.md),
which ordered `STORY-P1-07-05` started immediately (Red first) as the bridge between compiled
AArch64 code and a demonstrable product. This session delivered the Story's host half end to
end. **No board was touched; no hardware claim is made.** The next session with a board and a
loopback-tested adapter runs one command and reads one of five exit codes.

## What exists now that did not this morning

```text
cargo run -p xtask -- pi5 --fixture=boot [--port=COM3] [--baud=] [--timeout-secs=] [--quiet-secs=] [--board-rev=] [--firmware=]
```

1. **The workspace links an AArch64 binary for the first time.** New crate
   `os/src/pi5-image/` — packaging *only*: it links `hal_arm64::boot` for its `global_asm!`
   side effect, supplies a panic handler that parks, and is an inert stub on any other
   architecture. `#![forbid(unsafe_code)]`; every behaviour stays host-tested in `hal-arm64`.
   The CI step that built only the `hal-arm64` library now links this image, closing the
   "compiles but never links" gap the divergence record warned about (memcpy/memcmp).
2. **`kernel8.img` is built in-process, not by folklore.** `xtask` flattens the ELF itself
   (a tested pure parser, no `objcopy` dependency) and *validates* what the layout check in
   Handover 23 could only observe: entry point = lowest `PT_LOAD` = `0x80000`, first bytes
   `A4 00 38 D5`. First real build: 82,916 bytes. The printed placement carries `os_check=0`,
   `kernel=kernel8.img`, the 3-pin-connector and 115200-8N1 facts — each pinned by a test.
3. **The board now speaks the existing verdict protocol.** `hal-arm64`'s boot path emits
   `TINYOS-RESULT/1 fixture=boot ok=<bool>` after its vector install, self-checked on the two
   facts it can observe (EL1 reached; `VBAR_EL1` readback matches). Host tests on both sides
   cross-pin the exact bytes; the host consumes them with the *same* `timing::parse_result`
   the Tier 0 gate uses. No second parser, no new protocol — `LE-06`'s lesson held.
4. **Five pairwise-distinct exit codes.** 0 pass / 1 the board's own verdict said failure /
   2 harness error — unchanged Tier 0 meanings — plus `3` **silence** (not one byte) and `4`
   **spoke-without-verdict** (bytes, then nothing trustworthy; a corrupt verdict line is a
   failure to read a verdict, never a verdict). The capture loop is bounded (1 MiB, `SEC-20`),
   the quiet window arms only after the first byte, and every timeout decision is tested
   under a scripted clock. The serial-port open is the only untested I/O, kept as thin as it
   reads (reader thread + channel, `mode`/`stty` for line settings).
5. **Registration and attribution.** `list-fixtures` prints the `pi5` namespace beside the
   Tier 0 and measurable ones (manual, never CI — §7.4 decision b). Every run writes
   `capture.log` + `record.tsv` (commit, fixture, port, baud, operator-supplied board
   revision/firmware or "unrecorded", image and capture SHA-256, capture end reason, outcome,
   exit code, timestamp) under `os/target/pi5/runs/`.

## Evidence and discipline

- TDD: both crates' tests were written first and observed failing (compile-stage Red on
  `hal-arm64`, `unimplemented!()` Red on `xtask`), then went Green: `hal-arm64` 115 → 118,
  `xtask` 210 → 241 (31 new). `cargo fmt`, `clippy` (host and AArch64 target), `check-spine-files`,
  `check-assurance-spine`, `check-crate-sizes` all green.
- Statuses moved everywhere the `LE-44` machinery checks, together: Story header, Feature
  table row, dashboard badge — `-05` is **In progress — host half Green, criteria 2 and 3
  need a board**; `TEST-P1-07-05-A` is **Partially Verified (Host), 2026-07-30**, its
  specification untouched.
- `SEC-14`/`SEC-19`/`SEC-20`, `BND-03`/`PD-12` honoured as the Test document maps them; the
  assurance contract row stays `specified` until a live capture backs it, matching `-01`.

## What is deliberately not here

- **No fault fixture yet.** The `pi5` namespace holds `boot` only. The deliberate-exception
  demo is `-02`'s board criterion; adding its fixture is a small follow-on *when a handler
  strategy for resuming/reporting a deliberate fault is chosen*, and doing it speculatively
  today would be board logic with no test able to touch it.
- **No CI hardware, no deploy loop, no Ethernet/RP1/PCIe, no SD driver** — all restated from
  the Story's named debt; `LE-26` stays routed-around; `LE-09` stays open (a byte on a serial
  line is not a measurement).
- **`--port` capture is written but unexercised against a physical adapter.** Criteria 2 and 3
  close on a real capture, not on this session's word.

## For the board session (the demo 08A §3.1 wants)

1. Loopback-test the adapter (`TEST-P1-07-01-A` clause 1 — runnable before anything else).
2. `cargo run -p xtask -- pi5 --fixture=boot` → place `kernel8.img` + `config.txt` as printed.
3. `cargo run -p xtask -- pi5 --fixture=boot --port=COM<n> --board-rev=<sticker>` and
   power-cycle when told. Exit 0 with the transcript showing `current_el=` first, the READY
   sequence, `vbar … match=yes`, `TINYOS-RESULT/1 fixture=boot ok=true` **is** the -01 board
   half plus the vector-install half of -02's evidence, captured attributably. Exit 3: check
   adapter, connector muxing, `os_check=0` — in that order (divergence record §§1–4).

## Concurrency note

The soak logger owned `goals/reports/_soak-p0-03-01.log` throughout; it was left unstaged.
No other session's edits were observed mid-turn. Staged narrowly per
`agent/CONCURRENT_SESSIONS.md`; no shared spine TSV was modified.
