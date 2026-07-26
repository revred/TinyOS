# Handover 05 — Goals Progress Dashboard (`goals/index.html`)

Session date: 26 July 2026
Follows: [04-seedmvp-agentmd-goals-vv-model-handover.md](04-seedmvp-agentmd-goals-vv-model-handover.md)

## What changed

`goals/README.md` replaced with **`goals/index.html`** — a visual progress dashboard for the V&V model, so Epic/Feature/Story/Test status is scannable at a glance rather than requiring a read of the individual Markdown files. All content previously in `goals/README.md` (the model explanation, ID scheme, status lifecycle, just-in-time decomposition rationale, folder layout) is preserved in the new page, plus:

- Summary stat tiles (Epics decomposed, Features, Stories planned, Tests verified) and a progress bar.
- A live-status table for `EPIC-P0`'s Features, Stories, and Tests with color-coded status badges (Planned / In Progress / Blocked / Verified).
- A backlog summary table for `EPIC-P1` through `EPIC-P8`.

Every `goals/*.md` file that linked to `goals/README.md` (`epics/backlog.md`, `epics/EPIC-P0.md`, `features/FEAT-P0-02.md`) updated to link to `goals/index.html` instead, using anchor IDs added to the relevant sections (`#model`, `#status-lifecycle`, `#jit-decomposition`). `SeedMVP.md` Section 12 updated similarly.

## Key decision

**"Live" means manually-updated snapshot, stated honestly.** The dashboard is a static HTML file with no build step or server behind it — it is explicitly labeled as a snapshot that must be updated in the same PR that changes any Epic/Feature/Story/Test status, per `goals/traceability-matrix.md`'s existing sync rule, rather than implying real-time auto-refresh it can't actually provide. This is the same honesty principle already applied elsewhere in the project (e.g. the Apple Silicon hardware caveat in `docs/universal-driver-model.md`) — don't oversell what a static file can do.

## Documents touched

- Removed: `goals/README.md`
- New: `goals/index.html`
- Updated: `goals/epics/backlog.md`, `goals/epics/EPIC-P0.md`, `goals/features/FEAT-P0-02.md`, `SeedMVP.md`, this date's Handover 04 (historical note added)

## Next handover

None yet filed past this point at time of writing. See [`index.html`](index.html) for the running index of this date's handovers.
