# Test Cases: 5-Axis Motion Controller Workload

Status: **working draft — promote to `goals/tests/` entries once a corresponding Story exists**

Each case below is written in given/when/then form, matching the style already used in [`goals/tests/`](../../goals/tests/) (see `TEST-P0-01-01-A` for the pattern), so promoting a case here into a real `TEST-*` entry is a copy-and-link operation, not a rewrite.

## TC-1 — Linear and circular interpolation blending

**Given** a program with three consecutive linear/circular blocks forming a smooth path with no exact-stop directive,
**when** the interpolation service executes them,
**then** the tool path blends across block boundaries without a full velocity stop, staying within a defined path-deviation tolerance at each corner.

## TC-2 — Work coordinate system switch

**Given** a program that selects work coordinate system offset #2 partway through execution,
**when** the block containing the offset change executes,
**then** all subsequent positions are computed relative to offset #2 until another offset is selected, and the reported work-coordinate position display updates accordingly (machine-coordinate position is unaffected).

## TC-3 — Tool length/radius compensation toggling

**Given** a program that engages tool length compensation for tool #4, then later engages radius compensation for the same tool,
**when** both are active simultaneously,
**then** the computed tool-tip path reflects both compensations correctly, and disabling either one independently returns the path to the expected uncompensated-in-that-axis behavior.

## TC-4 — RTCP: tool-tip stationary during pure reorientation

**Given** a 5-axis program segment that reorients the tool (rotary-axis motion only, no programmed linear tool-tip motion) with RTCP active,
**when** the rotary axes move,
**then** the tool tip's position in work coordinates remains stationary (within tolerance) while the linear axes compensate for the rotary motion — this is the core RTCP correctness property and the highest-value test in this set.

## TC-5 — Simultaneous 5-axis contouring near a kinematic singularity

**Given** a programmed path that passes near (but not through) the trunnion table's kinematic singularity region,
**when** the interpolation service computes the required axis velocities,
**then** it either completes the path within each axis's velocity/acceleration limits, or reports a defined, ACI-visible error/warning rather than silently producing an infeasible or discontinuous axis command — behavior to be confirmed against the reference machine per `requirements.md`'s open items.

## TC-6 — Feed override during active motion

**Given** a program executing a linear move at programmed feed rate,
**when** the operator adjusts the feed override control mid-move,
**then** the actual feed rate changes proportionally without discontinuity in the resulting path, and the override adjustment is logged with full ACI provenance (who, what, when).

## TC-7 — Single-block mode halts correctly between blocks

**Given** single-block execution mode is engaged,
**when** the current block completes,
**then** motion halts at the block boundary and does not proceed to the next block until an explicit operator confirmation, with tool compensation and work-coordinate state preserved correctly across the halt (per `requirements.md`'s open item on this).

## TC-8 — Motion fault resolves to safe hold

**Given** a simulated following-error fault (via the `PositionFeedback` trait's Tier 0 simulated implementation) injected mid-program,
**when** the fault exceeds the defined tolerance,
**then** the system transitions to a documented safe-hold state, logs the fault through the ACI, and does not silently continue executing the program — mirroring the chaos/fault-injection testing approach already specified in [`SeedMVP.md`](../../SeedMVP.md#6-testing-strategy) Section 6.

## Cross-reference to `goals/`

None of these are yet promoted to `goals/tests/TEST-*` entries — per `goals/README`'s (now `goals/index.html`'s) just-in-time decomposition principle, that happens when a `motion`-related Feature/Story is created under an active Epic. TC-1 through TC-4 are the highest-priority candidates for that promotion, since they verify the core RTCP correctness property (G-PA-8) the CNC flagship demonstration depends on.
