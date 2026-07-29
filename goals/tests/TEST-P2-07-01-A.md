# TEST-P2-07-01-A — `.TCB` Batch Fixture + Golden-Transcript Parity Gate

Status: **Specified — written before the code, per the TDD mandate**
Story: [`STORY-P2-07-01`](../stories/STORY-P2-07-01.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D23`, `D14`
Security controls: `SEC-05`, `SEC-06`, `SEC-07`, `SEC-14`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-13`, `BND-14`, `BND-17`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-03`, `PD-05`, `PD-14`
Code admission gates: `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** the `shell` crate's `shell-batch` fixture built against
`os/targets/x86_64-tinyos.json` — a seeded labelled RAM volume, a session policy granting the
MVP verb set minus one deliberately withheld verb, and the embedded parity `.TCB`,
**when** it boots under QEMU via `xtask qemu-x86_64 --fixture=shell-batch` with serial capture,
**then**:

1. The transcript on COM1 is **byte-identical** to the committed golden file
   (`os/src/shell/golden/parity-smoke.golden.txt`) — checked by
   `cargo run -p xtask -- check-shell-parity`, which fails on the first divergent byte and
   prints both lines.
2. The fixture exits through `isa-debug-exit` with the success code only if its own in-guest
   assertions held (denied verb refused-and-audited, label carried across `COPY`, capacity
   exhaustion refused cleanly); the harness requires **both** signals, exactly as
   `timing.rs` does — a transcript that matches with a failing exit (or vice versa) is a
   harness error, never a pass.
3. Two consecutive boots produce byte-identical transcripts (determinism, acceptance 4 of
   the Story).

The golden file is the **parity oracle**: its content encodes the terminal-gap register's
decided output shapes (4.0 message strings where adopted, recorded divergences where
exceeded). Changing shell output means changing the golden file in the same commit, and the
diff **is** the review.
