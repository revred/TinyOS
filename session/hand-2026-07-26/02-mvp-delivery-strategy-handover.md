# Handover 02 — MVP Delivery Strategy and Workspace Layout

Session date: 26 July 2026
Follows: [01-initial-handover.md](01-initial-handover.md)

## What changed

Added [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md): the concrete "how the code gets built" companion to the Roadmap and to Section 10 (Roadmap Alignment) of the seed specification — a full Cargo workspace crate map, custom bare-metal target specs, `xtask` build/deploy tooling, and a phased delivery sequence.

## Key decisions made this handover

- **Concrete workspace layout decided:** all code — every crate, no exceptions — lives under `os/src/` (kernel, hal, drivers, aci, shell, motion, inference, compute, bridge-device/bridge-host, wci, deploy-device/deploy-client, config, xtask), with `os/targets/` for custom bare-metal target specs as the one sibling exception (build data, not source).
- **Naming collision resolved:** the README's originally planned `/agent/` code crate (for LLM integration) collided with the new root-level `agent/` guidelines folder holding `CODING_STANDARDS.md`. Resolved by renaming that crate to `os/src/inference/`, matching the "Agentic Inference" goal category name already used in the seed specification.
- **`xtask`, not shell scripts.** Build/test/QEMU-launch/deploy orchestration is an ordinary Rust `std` binary (`os/src/xtask/`), consistent with the Rust-primary language policy — no `.sh`/`.ps1` scripts as an undocumented, platform-specific side channel.
- **Delivery strategy is "walking skeleton first":** the first milestone is proving the empty build → QEMU-boot → CI-green pipeline end to end, before any real kernel feature work — with governance gates (crate-size CI check, SOLID review, TDD) active from the very first PR rather than bolted on once "real" development starts.
- **Full crate map published**, tying every planned crate to a Roadmap phase, its `no_std`/`std` classification, and its `unsafe` policy — see the table in `docs/mvp-delivery-strategy.md`.
- **The CNC flagship milestone is a cross-phase integration point**, not a single Roadmap phase — `motion` grows across Phases 0 through 3 (scheduler timing, shell/G-code front-end, and connectivity all have to land first), tracked as an explicit milestone checkpoint rather than an implicit side effect of phase completion.

## Documents touched

- New: `docs/mvp-delivery-strategy.md`
- Updated: `README.md` (Repository Layout section rewritten to the concrete `os/src/` structure)
- Updated: `SeedMVP.md` (Section 12 cross-reference index)

## Next handover

See [03-cnc-kinematics-merge-handover.md](03-cnc-kinematics-merge-handover.md) for a small follow-on correction to this crate map.
