# STORY-P1-07-05 — Host-Side Run Path: SD Image, Serial Capture, and the Same Exit-Code Scheme

Status: **In progress — host half Green 2026-07-30 (31 host tests, Red first; the one command builds `kernel8.img` and prints placement); criteria 2 and 3 await a live board capture through this same path. Not Verified.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md) §5, piece 5 of the `LE-09` slice

## Description

Piece 5: `cargo run -p xtask -- pi5 --fixture=...` builds an image, reports how to place it on the SD card, captures serial, and exits with the **same code scheme as `qemu-x86_64`**.

The point is not convenience. Hardware evidence that no tool can reproduce is anecdote, and a second, divergent harness beside the Tier 0 one is the shape `LE-06` already cost this project once (`pool-bench`, a divergent sibling harness, closed by folding it back in). The UART-borne pass/fail protocol this path consumes already exists — it shipped inside `STORY-P1-01-02` precisely because *a gate that can only read a QEMU exit code can never gate hardware*. This Story is where that foresight is either collected or found to have been wrong.

**No Ethernet.** [Handover 08 of 27 July](../../session/hand-2026-07-27/08-epic-p1_5-deploy-loop-transport-decision.md) recorded peer-to-peer Ethernet as the near-term dev-loop transport. On a Pi 5, USB, Ethernet and GPIO all sit behind the RP1 southbridge over PCIe, so that decision now implies PCIe bring-up plus an RP1 driver plus a NIC driver before one byte can be deployed — a Feature of its own, not a transport. That is `LE-26`, raised by this plan and routed around here rather than answered.

## Depends on

`STORY-P1-07-01` (there is no image to place and no serial to capture without it). Independent of `-02`, `-03` and `-04`, and may be built in parallel with them.

## Acceptance criteria

1. **One command builds a placeable image** for the target spec `STORY-P1-07-01` committed, and prints exactly where the artifacts go on the boot partition. **Automating the physical SD swap is out of scope** — manual swap is acceptable and expected. The command's job is that nothing about the image is folklore.
2. **Serial capture drives the exit code, using the existing UART pass/fail protocol with no new protocol invented.** Same code scheme as `qemu-x86_64`, so a reader who trusts the Tier 0 path already knows how to read this one.
3. **Timeout and silence are distinguishable failures, not one hang.** A board that never speaks, a board that speaks and stops, and a board that reports failure are three different outcomes and must exit differently. On this hardware silence is the *common* case during bring-up, and a run path that treats it as "still working" wastes the session it was built to save.
4. **The path is registered the way every other fixture is** — it appears in `list-fixtures` with its owning `TEST-*`, per `STORY-P0-01-04`'s rule that a fixture nobody runs is an unverified fixture that looks verified.
5. **The host-side logic is unit-tested without a board.** Capture parsing, verdict extraction, timeout handling and exit-code mapping are pure functions over captured text. Only the serial port open is I/O, and it is the seam.

## Named debt this Story leaves open

- **No CI integration.** Per the recorded §7.4 decision (option (b), [Handover 19](../../session/hand-2026-07-28/19-feat-p1-07-acceptance-and-spine.md)), hardware runs stay manual and land in Reports; CI stays Tier 0. The ratio baselines therefore stay Tier 0 and `LE-23` is unaffected either way.
- **No deploy loop.** This is a bring-up run path, not `EPIC-P1_5`'s deploy tooling, and it makes no signing, atomicity or rollback claim.
- `LE-26` is raised, not closed.

## Progress, 2026-07-30

Split along the line the board draws, the way `-01` and `-02` split before it.
**The half that needs no hardware is done and Green on the x86_64 dev machine;
the live capture through a real serial port has not happened, and this Story is
not Verified.**

| Criterion | State |
|---|---|
| 1 — one command builds a placeable image and prints placement | **Green, and executed.** `cargo run -p xtask -- pi5 --fixture=boot` linked the first AArch64 binary in this workspace's history (`pi5-image`, packaging only — every behaviour stays host-tested in `hal-arm64`), flattened it in-process (no `objcopy`; the ELF parser is a tested pure function that *validates* entry = load address = `0x80000`), and produced an 82,916-byte `kernel8.img` whose first bytes are the divergence record's pinned `A4 00 38 D5`. The placement text carries `os_check=0`, `kernel=kernel8.img` and the 3-pin-connector/115200 facts, each pinned by a test. |
| 2 — serial capture drives the exit code, existing protocol | **Half.** The verdict is read by the *same* `timing::parse_result` the Tier 0 gate uses (no second parser, no new protocol), and `hal-arm64`'s boot path now emits the `TOS64-RESULT/1` line after its vector install, self-checked (EL1 reached, `VBAR_EL1` readback matches) — both sides pinned by cross-tests. The live half — a real port, a real board — is the open half. |
| 3 — timeout and silence are distinguishable failures | **Half.** Silence (exit 3), spoke-without-verdict (exit 4), reported failure (1), pass (0) and harness error (2) are five pairwise-distinct process exits; the capture loop (bounded at 1 MiB, `SEC-20`) distinguishes never-spoke / spoke-and-stopped / verdict-seen under a scripted clock. The live half is the same open half as criterion 2. |
| 4 — registered like every other fixture | **Green.** `list-fixtures` prints the `pi5` namespace with its owning `TEST-*`, beside (not inside) the Tier 0 and measurable namespaces. |
| 5 — host-side logic unit-tested without a board | **Green.** 31 host tests, written Red first: classification, exit mapping, capture bounds/endings, ELF flattening (including truncation, wrong-arch, wrong-address and oversize rejection), placement text, run-record rendering, SHA-256 against FIPS vectors. The serial-port open is the only I/O and is the seam. |

Every run writes an attribution record (`TEST-P1-07-05-A` clause 7): commit,
fixture, port, baud, operator-supplied board revision and firmware version
("unrecorded" rather than omitted), image and capture SHA-256, capture end
reason, outcome, exit code, timestamp — beside the raw capture.

## Tests

[`TEST-P1-07-05-A`](../tests/TEST-P1-07-05-A.md) — written before implementation, per the TDD mandate.
