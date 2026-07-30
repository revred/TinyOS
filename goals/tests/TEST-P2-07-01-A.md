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
4. **The third signal** (`STORY-P2-07-02`, closing `LE-56`): after the transcript the
   fixture emits one `TINYOS-SPOOR/1 len=<n> denials=<n>` marker line — the length of the
   in-guest spoor journal into which a decorator policy stamped every verb denial as a
   kernel `Spoor`, and the batch runner's own denial counter. `check-shell-parity` splits
   the capture at the marker (the transcript before it stays byte-sacred for signal 1),
   requires the trailer present, well-formed and self-corroborating (`len == denials`),
   and names all three facts in its success line. A missing or malformed trailer is a
   **FAIL, never a skip**. The in-guest assertion additionally requires
   `spoor_journal_len == denials == expected_denials()` before the success exit (signal
   2's half of the same fact), and the transcript itself shows the journal via the `SPOOR`
   verb — the golden encodes the spoor row.

The golden file is the **parity oracle**: its content encodes the terminal-gap register's
decided output shapes (4.0 message strings where adopted, recorded divergences where
exceeded). Changing shell output means changing the golden file in the same commit, and the
diff **is** the review.
