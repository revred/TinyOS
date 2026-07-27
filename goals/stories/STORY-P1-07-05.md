# STORY-P1-07-05 — Host-Side Run Path: SD Image, Serial Capture, and the Same Exit-Code Scheme

Status: **Specified — not started; needs `TEST-P1-07-05-A` Red first**
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

- **No CI integration.** Per the recorded §7.4 decision (option (b), [Handover 18](../../session/hand-2026-07-28/18-feat-p1-07-acceptance-and-spine.md)), hardware runs stay manual and land in Reports; CI stays Tier 0. The ratio baselines therefore stay Tier 0 and `LE-23` is unaffected either way.
- **No deploy loop.** This is a bring-up run path, not `EPIC-P1_5`'s deploy tooling, and it makes no signing, atomicity or rollback claim.
- `LE-26` is raised, not closed.

## Tests

[`TEST-P1-07-05-A`](../tests/TEST-P1-07-05-A.md) — written before implementation, per the TDD mandate.
