# STORY-P0-01-04 — The Harness Is Held to the Discipline It Enforces

Status: **Functionally Verified (Tier 0 + Host), 2026-07-28** — assurance state `baseline-debt`
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-28/16-next-session-mandate.md`](../../session/hand-2026-07-28/16-next-session-mandate.md)
Implemented in: [`session/hand-2026-07-28/17-story-p0-01-04-harness-assurance.md`](../../session/hand-2026-07-28/17-story-p0-01-04-harness-assurance.md)

## Description

Retire the assurance debt in `xtask`'s own tooling, and close the exit-code hole two fixtures have carried across three mandates. `FEAT-P0-01` owns `xtask`, CI and the exit-code discipline, and its containment contract already requires that *"CI must reject incomplete class, Feature, Story, security-control, or boundary-test contracts"* — so the tooling that performs that rejection belongs here.

## Depends on

`STORY-P0-01-02`, `STORY-P0-01-03`.

## Acceptance criteria (final)

1. **A panicking TinyOS binary says so on the UART before it stops.** **Met**: `hal_x86_64::qemu_exit::panic_report` emits `TINYOS-PANIC/1 message=… file=… line=…` and then `exit_qemu(Failure)`. All **eleven** `#[panic_handler]`s in the workspace were byte-identical bare `exit_qemu` calls and now route through it, so the behaviour is shared rather than copied. Every write is `let _ =`: a UART that will not accept bytes must not be able to turn a panic into a hang.

2. **An unrouted interrupt is diagnosable.** **Met**: `hal_x86_64::interrupts::unhandled_interrupt_handler` emits `TINYOS-UNROUTED/1 fail_closed=true` before the same fail-closed stop. The containment action is unchanged; only a diagnostic was added.

3. **Both fixtures' CI steps assert on content.** **Met**: `broken-boot` greps for its own deliberate panic message and `idt-apic-unrouted` for the fail-closed sentinel, each alongside the exit-code check — the shape `wcet-trip` and `os-runaway` already use.

4. **Every fixture's declared owning Test exists.** **Met**, and it passed on the first run — the registry's `owning_test` column was accurate.

5. **Every fixture is actually run by CI.** **Met, after the finding below.** Coverage is credited by *build target* rather than by name, because `--fixture=` is overloaded across two namespaces and one fixture is spelled `dispatch` in one and `dispatch-measure` in the other; comparing names would have reported a covered fixture as unrun.

6. **The tooling shipped in `2da1ccd` has its assertions written down.** **Met, and labelled for what it is.** The loose-ends register, status grammar and fixture registry were already covered by 25 unit tests including every violation case. What they lacked was the assurance artifact. `TEST-P0-01-04-A` clause 6 enumerates the properties; clause 7 states plainly that this clause is retrospective and not TDD, because blurring that is how the discipline erodes.

## The finding this Story produced

**Nine of the twenty-three Tier 0 fixtures existed, compiled, passed — and no CI step ran any of them.**

```
context-switch, idt-apic-timer, idt-apic-unrouted, pci-enumeration,
address-space, win32-shim, blue-sharc, blue-sharc-broken, shared-memory
```

Each is named by an owning Test document that claims Tier 0 evidence for it. That evidence was last produced on somebody's development machine, at an unknown commit, and nothing has re-established it since. **An unrun fixture is an unverified fixture that looks verified** — `LE-07`'s lesson (CI unobserved for thirty handovers) arriving in a new place, and it went unnoticed because the existing drift guard only checked CI → table. The dangerous direction was the unguarded one.

All nine pass, so this is a gap in evidence rather than in behaviour. It is now guarded in both directions by a host test.

**The second finding is why the exit-code hole survived three mandates.** It was recorded as "the CI step should grep the serial capture", which sounds like a CI edit. Running the fixtures showed **both produce an empty capture**: `broken-boot`'s panic handler was a bare `exit_qemu` and so was the unrouted-interrupt default. There was nothing to grep, and the work was never as small as its description. **A TinyOS kernel that panicked died in complete silence** — a test-harness gap was hiding a system defect, and on a board, where there is no exit code at all, that silence is the whole diagnostic.

## Tests

[`TEST-P0-01-04-A`](../tests/TEST-P0-01-04-A.md) — clauses 1–5 written before implementation and driven Red-to-Green; clause 6 is retrospective and says so.

## Reports

- [`REPORT-2026-07-28-07`](../reports/REPORT-2026-07-28-07.md) — the empty-capture finding, the nine unrun fixtures, the Red runs and the new pass conditions.

## Goals verified

G-DX-3, G-DX-7. Neither is closed.

## Named debt this Story leaves open

- **New: `LE-25`** — `unhandled_interrupt_handler` cannot name the vector it caught. It is one shared stub installed for every unrouted vector and receives no vector argument; naming it needs per-vector trampolines. The sentinel proves containment was reached, not which vector reached it.
- **`LE-07` reinforced, not closed.** The guard added here catches a fixture that is never run. It does not catch a fixture that is run and whose result nobody reads.
