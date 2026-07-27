# agent.md — Entry Point for Any Coding Agent

This file is the single entry point for any LLM-based coding agent working in this repository — Claude, GPT, Gemini, or otherwise, whether invoked via a CLI tool, an IDE extension, or an autonomous pipeline. It plays the same role a tool-specific file (e.g. `CLAUDE.md`) plays for one assistant, but is written to be tool-agnostic: nothing here assumes a specific vendor or product. If your tooling looks for a particular filename and you can only symlink or copy one file, make it this one.

Not to be confused with [`agent/`](agent/) (a directory) — that folder holds [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md), the binding coding rules this file points you to below. `agent.md` is a file at the repository root; `agent/` is a folder. Both exist; they are not the same thing.

## Read these, in order, before writing anything

1. **[`SeedMVP.md`](SeedMVP.md)** — Section 1 is the fixed founding intent (read it first, it's six sentences). The rest is the comprehensive master specification: goal taxonomy, hardware catalog, MVP narrowing, testing strategy, reliability and security guarantees, and codebase governance. If you're about to make a nontrivial design decision, check whether it's already been made here.
2. **[`SECURITY_CHARTER.md`](SECURITY_CHARTER.md)** — the governing Protection Domain, cross-class communication, and remote-code exclusion charter. Any work that parses external bytes, maps executable memory, launches a process, crosses a domain, exposes a remote path, deploys code, or grants authority must satisfy its machine-readable `PD-*` and `RCG-*` contracts.
3. **[`docs/whole-system-context.md`](docs/whole-system-context.md)** — the destination architecture that keeps goals, the 625 performance guardrails, concrete applications, and security/containment side by side. It distinguishes current, next, later, and research horizons for runtimes, games, networking, browser, TinySpot, TLE/WST, fleets, and the browser-hosted lab.
4. **[`README.md`](README.md)** — the current-state living design document. If it disagrees with anything in `SeedMVP.md`, the README wins for day-to-day details; `SeedMVP.md` wins for founding intent and governance rules. Neither may weaken the Security Charter.
5. **[`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md)** — binding, not advisory. Language policy, `unsafe` boundaries, real-time coding discipline, the crate-size ceiling, SOLID enforcement, mandatory TDD, and the priority ordering that resolves every trade-off. Every PR is held to this.
6. **The latest dated folder under [`session/`](session/)** — sorted by date descending, its `index.html` indexes that date's handovers in order. Read the most recent one for a snapshot of what's decided, what's open, and what to work on next, before you assume you know the current state.
7. **[`goals/`](goals/)** — the verification & validation model: Goals → Epics → Features → Stories → Tests → Reports, with the 625 performance tests, 20 security controls, five containment classes, 14 Protection Domain contracts, 14 code-admission gates, complete 25-pair class matrix, 19 application/platform targets, and nine landing zones forming the mandatory [assurance spine](goals/assurance/README.md). If you're implementing something, find (or create) the parent Feature's row in `goals/assurance/feature-contracts.tsv` and the Story's row in `goals/assurance/story-contracts.tsv` before you write code.

## The rules that never bend

These are restated here because they are the ones most likely to be sacrificed under time pressure — see [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#priority-ordering) for the full statement.

1. **Safety before security before correctness before performance.** In that order, always. Never reorder this to hit a deadline.
2. **No privileged bypass for any caller.** Not the local shell, not a remote host, not an LLM agent — including you. Every action goes through the Agent Command Interface (ACI) policy engine described in the README.
3. **Test-driven, no exceptions.** A failing test exists before the code that makes it pass. If you are about to write implementation code with no corresponding test, stop and write the test first.
4. **No crate exceeds 20,000 lines of code, excluding tests.** If you're approaching that limit, split the crate — do not ask for an exception; there isn't one.
5. **SOLID principles are reviewer-enforced and blocking**, not aspirational — see `agent/CODING_STANDARDS.md` for the Rust-specific translation of each principle.
6. **Fail-safe over keep-trying**, everywhere. A fault, a dropped connection, a stalled inference request — all resolve to a safe state, never an infinite retry against a real-time deadline.
7. **All code lives under `os/src/`.** Every Rust crate, every workspace member — see [`docs/mvp-delivery-strategy.md`](docs/mvp-delivery-strategy.md) for the full crate map. Nothing compiled belongs loose at the repository root.
8. **No Feature or Story bypasses the assurance spine.** Every Feature declares implementation/subject containment classes, hostile inputs, authority posture, and `BND-*` tests in [`goals/assurance/feature-contracts.tsv`](goals/assurance/feature-contracts.tsv). Every Story selects performance domains, security controls, and containment classes in [`goals/assurance/story-contracts.tsv`](goals/assurance/story-contracts.tsv). Functional verification does not erase missing timing, frugality, isolation, signing, boundary, adversarial, or hostile-load evidence.
9. **Remote bytes are data, never code.** No network, host, shell, model, file, debug, deploy, or compatibility path may create executable memory except through every gate in [`goals/security/code-admission-gates.tsv`](goals/security/code-admission-gates.tsv). C4 inspection is destroyed; admitted code starts as a fresh C3 domain with empty authority.
10. **Every product destination stays joined across four planes.** New runtimes, applications, compatibility layers, browsers, protocols, or fleet roles update [`goals/context/application-platforms.tsv`](goals/context/application-platforms.tsv) and [`landing-zones.tsv`](goals/context/landing-zones.tsv) with their goal, performance, security, class, horizon, and claim-gate selections before implementation. Runtime permissions never replace the OS Protection Domain.

## How to orient quickly on a specific task

- **Implementing a feature?** Find its Story under [`goals/stories/`](goals/stories/) (or its Epic/Feature if the Story doesn't exist yet — decompose just-in-time, don't pre-build the whole tree), add its assurance contract, and run `cargo run -p xtask -- check-assurance-spine` from `os/`. Check which crate it belongs to in [`docs/mvp-delivery-strategy.md`](docs/mvp-delivery-strategy.md). Write the test first.
- **Touching communication/security-sensitive code (HBP, WCI, deploy, ACI, executable mapping, process launch, or domain teardown)?** Read [`SECURITY_CHARTER.md`](SECURITY_CHARTER.md) and the relevant spec under `docs/` in full before changing anything. These subsystems require adversarial tests, not just happy-path coverage.
- **Unsure whether something's already been decided?** Check `SeedMVP.md` Section 12 (Cross-Reference Index) for the full document map, then the specific spec it points to. Don't re-derive a decision that's already written down.
- **Making a judgment call with no obvious answer?** Apply the priority ordering (safety > security > correctness > performance) and document the reasoning in your commit message or PR description — future readers (human or agent) shouldn't have to guess why.
- **Finishing a unit of work?** If it changes the project's decided state (not just in-progress code), a session handover may be warranted — see [`session/README.md`](session/README.md) for the convention.

## What this file is not

It is not a replacement for `SeedMVP.md`, `README.md`, or `agent/CODING_STANDARDS.md` — it is a map to them. Don't duplicate their content here as this file drifts out of sync with the documents it points to; keep it short and update the links, not the substance.
