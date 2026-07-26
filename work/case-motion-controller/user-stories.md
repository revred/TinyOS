# User Stories: Real-Time G-Code Motion Control on TinyOS

Status: **working draft — the flagship demo case for the 5-axis CNC workload**

Informed by general, publicly-known conversational/shop-floor CNC programming conventions (the category of tooling sometimes called "conversational programming" or "shop-floor programming assistants," a well-established product class across CNC vendors) and by [`requirements.md`](requirements.md)/[`test-cases.md`](test-cases.md) in this folder. See [`references.md`](references.md) for the reference manuals used — cited by title, not reproduced.

## Why this is the flagship demo case

Every other TinyOS deployment mode is either invisible to a non-technical observer (inference tokens over a secure channel) or requires physical hardware most audiences can't watch safely up close (a co-bot, a rocket landing pod). **Live G-code motion control is different: it's immediately legible.** An operator types or pastes a G-code block, the machine visibly moves, the position readout updates in real time, and every one of TinyOS's core claims — remote-first UX, ACI gating, fail-safe behavior, DOS/POSIX shell parity — is demonstrable in the same thirty seconds. This is why it's called out as the flagship demo, distinct from (though built on) the flagship *specification* work already defined for the CNC controller in [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md).

## User roles

- **Machine operator** — runs programs, jogs the machine, monitors execution. Typically holds `operator` ACI capability scope.
- **Programmer/setter** — writes or edits G-code programs, sets tool offsets and work coordinates, typically before an operator runs the job. May hold `operator` or `supervisor` scope depending on deployment policy.
- **Supervisor** — can override, pause, or take command authority from an operator session; holds `supervisor` scope.
- **Remote engineer** — connects over HBP or WCI from off-site to diagnose or adjust a program; subject to the exact same ACI gate as a local operator, per Design Pillar 2.

## Core user stories

### Real-time G-code execution

- **US-1.** As a machine operator, I want to load a G-code program and run it in automatic mode, so that the machine executes the full part program without me re-entering each block.
- **US-2.** As a machine operator, I want to see live machine-coordinate and work-coordinate position readouts updating as the program runs, so that I always know exactly where the tool is.
- **US-3.** As a machine operator, I want to enter a single G-code block via manual data input (MDI) and have it execute immediately, so that I can test or verify a single move without writing a full program.
- **US-4.** As a machine operator, I want single-block mode, so that I can step through a new or unfamiliar program one block at a time before trusting it to run unattended.
- **US-5.** As a machine operator, I want dry-run mode, so that I can verify a program's motion is correct without engaging the spindle or any process output.

### Real-time control while running

- **US-6.** As a machine operator, I want to adjust the feed override while a program is running, so that I can slow down through a tricky section or speed up through open air, without stopping and restarting the program.
- **US-7.** As a machine operator, I want an immediate, unambiguous way to pause execution and return to a safe hold state, so that I can react to something unexpected without hunting through a menu.
- **US-8.** As a supervisor, I want to take command authority from an operator's session (per the WCI-style authority-lease model already specified in `docs/wci-spec.md`), so that I can intervene directly if something looks wrong, without a race condition between two people issuing conflicting commands.

### Programming assistance

- **US-9.** As a programmer, I want to write G-code directly in `TINYCMD` (via either DOS or POSIX syntax front-end, per `docs/cli-compatibility-mvp.md`) using a plain-text editor workflow, so that I'm not locked into a proprietary program-entry UI.
- **US-10.** As a programmer, I want a conversational/shape-based programming aid (select a cycle — e.g. a pocket, a drilled-hole pattern, a facing pass — and enter its parameters, with G-code generated automatically) as a *layer on top of* raw G-code, not a replacement for it, so that common operations are fast to program without losing the ability to hand-edit the generated code. This is explicitly a **post-MVP** story — the MVP demo case is direct G-code entry and execution (US-1 through US-8); conversational programming is a stretch goal that reuses the same underlying interpreter and interpolation service, not a separate code path.
- **US-11.** As a programmer, I want tool offsets and work coordinate values editable from the same shell that runs programs, so that I don't need a separate tool for machine setup versus machine operation.

### Remote operation

- **US-12.** As a remote engineer, I want to connect over a secure channel (HBP if co-resident, WCI if networked) and observe the same live position/status telemetry an on-site operator sees, so that I can diagnose an issue without being physically present.
- **US-13.** As a remote engineer, I want any command I issue remotely to be gated by the exact same ACI policy engine as a local command, with full audit provenance, so that remote access never becomes a lower-scrutiny path into the machine.

### Safety

- **US-14.** As a machine operator, I want the physical e-stop to work regardless of what TinyOS's software is doing at the moment — mid-program, mid-network-hiccup, mid-anything — so that I never have to think about whether the safety system depends on software state.
- **US-15.** As a machine operator, I want a following-error or unexpected-feedback fault to stop the machine safely and tell me what happened, so that I'm never left guessing why the machine stopped.

## Flagship demo script (illustrative, not a committed feature list)

A concrete walkthrough for what "live G-code motion control" looks like as a demo, tying the stories above together — useful for validating that the stories are complete, not a spec in itself:

1. Remote engineer connects over WCI from a laptop, authenticates, and requests `operator` authority (US-12, US-13).
2. Operator (or the same remote session) enters an MDI block moving one axis a known distance; the position readout updates live (US-2, US-3).
3. A short program is loaded and dry-run to verify the toolpath (US-5), then run in single-block mode for the first few blocks (US-4), then switched to full automatic execution (US-1).
4. Feed override is adjusted mid-run to visibly change cutting speed without stopping the program (US-6).
5. A simulated fault is injected (per `test-cases.md`'s TC-8) — the system halts to a safe state and reports why (US-15), demonstrating the fail-safe behavior without needing to damage or risk real hardware to prove it.

## Cross-reference to `goals/`

None of these Stories are yet promoted to `goals/stories/STORY-*` entries. US-1 through US-8 (real-time execution and control) are the highest-priority candidates for promotion once a `motion`-related Feature exists under an active Epic — they're the direct user-facing expression of the RTCP/interpolation correctness already tracked as TC-1 through TC-4 in `test-cases.md`.
