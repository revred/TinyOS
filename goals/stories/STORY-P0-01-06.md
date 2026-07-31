# STORY-P0-01-06 — The `D09` Disposition: Which of 25 `Host+T0` Gates Are Actually Reachable

Status: **Verified (Tier 0 + Host), 2026-07-28** — assurance state `baseline-debt`; all 25 `D09` gates dispositioned, `PERF-D09-G20` closed on measurement, and `LE-09` shown to be the correct blocker for exactly one of them. This Story records evidence for another Story's gates and closes none of its own.
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-28/29-next-session-mandate.md`](../../session/hand-2026-07-28/29-next-session-mandate.md) §"What to do", item 2

## Description

`LE-31`, narrowed to the one slice that can be finished and that changes what every later session believes.

The register attributes `0 / 57 Stories assurance-verified` to `LE-09` — no hardware tier. [Handover 22](../../session/hand-2026-07-28/22-the-zero-is-real-but-its-reason-is-wrong.md) established that this is wrong for nine Stories, and [Handover 28](../../session/hand-2026-07-28/28-analysis-response-and-le-33.md) narrowed those nine to **one candidate that needs no hardware purchase at all**: `STORY-P0-05-01`, already functionally `Verified`, selecting `D09` alone, whose 25 release gates are every one of them tiered `Host+T0`.

Nobody has checked whether "`Host+T0`" is true. This Story checks it — for all 25, with a named blocker each — and builds the measurement for the subset that turns out to be reachable.

The expected result is deliberately not "the zero moves". It is that the *reasons* become specific enough to act on, and that the ones which are genuinely blocked are blocked by something a session can be assigned.

## Depends on

`STORY-P0-05-01` (functionally `Verified`; this Story records evidence against its domain), and `STORY-P1-01-01`'s shared `kernel::measure` harness.

## Acceptance criteria

1. **Every one of `D09`'s 25 gates carries a disposition and a named blocker.** `closeable-now`, `blocked-on-tooling`, `blocked-on-environment`, `blocked-on-hardware`, or `blocked-on-subsystem`. Restating `LE-09` as the blocker for a `Host+T0` gate is the specific error this Story exists to correct, and is not available as an answer.
2. **The `D09` work unit is measured at Tier 0 through the shared harness.** `exec::pe::parse` — both the accept path and the denial path — inside the real `x86_64-tinyos` binary under QEMU, emitting the versioned `TOS64-MEAS/1` envelope. A domain measured only on well-formed input has not been measured on the input `G20` is stated against.
3. **The gated timing baseline is not rewritten.** The fixture sits outside the gated `measure` envelope, per the `fixture-pool-bench` precedent. Re-recording the baseline on a Windows dev host would bake in the confirmed 23–53% cross-host offset (`LE-23`) and is one command from a false green (`LE-28`) — producing `D09` evidence by corrupting every other domain's baseline is a net loss.
4. **A gate is filed as evidenced only if the evidence meets its whole target.** Partial coverage leaves the gate absent from the register with its remainder named, following `STORY-P0-01-05`'s rule that the register is a count of evidence and never a score.
5. **`PERF-D09-G05`'s run-to-run stability requirement is measured and reported whichever way it falls.** The existing `measure` fixture's metrics have been observed between 1.48% and 81.38% p99 CV on the dev host, so a ≤ 5% requirement is a claim about the measurement environment before it is one about the code.

## Named debt this Story leaves open

- **`LE-09` is untouched.** Every number produced here is QEMU/TCG, and no Tier 0 number is hardware WCET evidence.
- **`STORY-P0-05-01`'s assurance state may not move**, and this Story does not promise that it will. A correct disposition is the deliverable.
- **The other eight Stories `LE-31` names are not audited here.** This is the one Handover 28 identified as needing no hardware; the rest remain `LE-31`'s.

## Tests

[`TEST-P0-01-06-A`](../tests/TEST-P0-01-06-A.md) — written before implementation, per the TDD mandate.
