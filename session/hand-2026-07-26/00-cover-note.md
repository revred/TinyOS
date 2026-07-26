# Cover Note — Session 2026-07-26

This note sits in front of [Handover 01](01-initial-handover.md) deliberately — numbered `00` so it sorts first — because it states a mandate the six handovers that follow don't individually spell out: **what "done" looks like for TinyOS's shell/utility surface, and how future work sessions should be resourced to get there fast.**

## The ambition: everything MS-DOS could do, plus what MS-DOS never had

TinyOS borrows MS-DOS's ergonomics deliberately (see [`README.md`](../../README.md#the-dos-inheritance)) — not as a nostalgia gesture, but as a genuine functional bar. The mandate is: **TinyOS's command surface should eventually match everything a DOS 4+ user could do at a prompt** — file and disk management, batch scripting, device configuration, text search/sort/paging, program execution, diagnostics — reachable through `TINYCMD`, exactly as familiar as the original, per [`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md).

And then TinyOS has to do something DOS structurally never could: **run all of it under a real preemptive, priority-based, multitasking kernel**, with every one of those commands ACI-gated identically whether it arrives from the local shell, a remote host over HBP/WCI, or an LLM agent — a security and determinism model that has no DOS-era analog at all. "Match MS-DOS's command surface" is the *floor*, not the ceiling; the RT core, the ACI, and remote-first operation are what make it a 2020s OS wearing a 1988 UX, not a DOS clone.

### Where that bar already is and isn't met

[`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md) already scopes the large majority of DOS's everyday command surface — `DIR`, `COPY`, `DEL`, `REN`/`MOVE`, `MD`/`RD`, `TYPE`, `FIND`, `SORT`, `MORE`, `TREE`, `ATTRIB`, `SET`, `ECHO`, `CLS`, `VER`, `VOL`, `MEM` — each bound to both DOS and POSIX syntax over one canonical verb core. What that document explicitly defers, and this cover note flags as **worth re-examining against the "everything MS-DOS could do" bar rather than leaving open-ended**:

- **A text editor** (an `EDLIN`-class minimum, not necessarily a full-screen one) — currently deferred with "file editing happens off-device for now." If the ambition is genuine DOS-prompt parity, this deserves a committed Roadmap slot, not an indefinite deferral.
- **Disk-level utilities** (`FORMAT`, `CHKDSK`, `DISKCOPY`, `LABEL`) — not yet in the MVP verb table at all; these belong with the storage class driver work in `EPIC-P3` (Connectivity) per [`goals/epics/backlog.md`](../../goals/epics/backlog.md), and should be added there explicitly rather than assumed.
- **Batch scripting depth** (`IF`/`FOR`/`GOTO`/`CALL`/`%1` — already scoped) is committed; a print-queue equivalent (`PRINT`) and code-page/keyboard layout commands (`KEYB`, `NLSFUNC`) are not, and are lower priority — noted here so their absence is a decision, not an oversight.

## The delivery mandate: subagents, and substantial output on the first real swipe

Everything produced so far (Handovers 01–06) is specification and case-study work — real, load-bearing, but not code. When Phase 0 implementation actually begins, the standing instruction from this cover note is:

**Don't deliver Phase 0 as a single-threaded trickle of one Story at a time.** [`goals/epics/EPIC-P0.md`](../../goals/epics/EPIC-P0.md) already decomposes into four Features (`FEAT-P0-01` through `FEAT-P0-04`); `FEAT-P0-01`'s three Stories are independently well-specified. The first real implementation work session should use **subagents working in parallel across those Features/Stories** — one agent per Story (or per test-writing/implementation pair within a Story, honoring the TDD mandate's red-before-green ordering) — so that session produces multiple Verified Stories at once, not one.

This is a mandate for *how the next session should be resourced*, not an instruction that's self-executing. Concretely, when that session starts:

1. Confirm hardware/toolchain readiness (MVP boards purchased or QEMU-only bring-up accepted as sufficient for Tier 0).
2. Use the Workflow tool's multi-agent orchestration — explicitly opted into by whoever runs that session, per this project's own tooling policy — to fan out `STORY-P0-01-01` through `STORY-P0-01-03` (and, once `FEAT-P0-01` is Verified, `FEAT-P0-02` through `FEAT-P0-04`) across parallel subagents, each writing its test first, then its implementation, then reporting a Result back to [`goals/reports/`](../../goals/reports/).
3. Update [`goals/index.html`](../../goals/index.html) and [`goals/traceability-matrix.md`](../../goals/traceability-matrix.md) in the same session, so "substantial" is verifiable from the dashboard, not just claimed in a handover.

"Substantial on the very first swipe" means: by the end of that session, `goals/index.html`'s progress bar should show more than 0% Verified, across more than one Story, because parallel subagents worked the walking-skeleton Features simultaneously rather than sequentially re-deriving context for each one.

## What this note does not do

It does not launch that work itself — no code exists yet in this repository, and starting Phase 0 implementation via subagents is a distinct, explicit action for whoever runs that session to trigger deliberately, not something a planning document should silently kick off. This note is the brief they should read first.
