# Handover 01 - STORY-P0-03-01 PERF-D07 assurance-evidence session

Follows: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md) (`EPIC-P0` functionally complete, `EPIC-P1` decomposed).

## What this session did

Picked `STORY-P0-03-01` (the `Pool<T,N>` bounded-capacity allocator, `os/src/kernel/src/mem.rs`) as the first Phase-0 Story to attempt real `PERF-D07` release-guardrail evidence, not just the structural assurance-spine scaffolding that already existed. Four pieces of work fed into this: a measurement design plan against all 23 non-Claim `PERF-D07` guardrails; a Host-tier diagnostic harness (6 new `#[cfg(test)]` functions in `mem.rs`, RDTSC/`Instant`-timed); a real Tier-0 QEMU fixture (`os/src/kernel/src/fixture_pool_bench.rs`, new `os/src/hal-x86_64/src/serial.rs` COM1 driver, wired into `xtask qemu-x86_64 --fixture=pool-bench`); and a `SEC-03`/`SEC-19`/`SEC-20`/`BND-04`/`BND-15`/`BND-20` boundary-evidence mapping. An independent adversarial verification pass then re-ran essentially everything from a clean invocation.

## The important correction this handover has to make plainly

**This did NOT close 22 of the 23 non-Claim `PERF-D07` guardrails.** That was the working assumption partway through this session, and it was wrong: the three earlier sub-pieces (design plan, Host harness, QEMU fixture) each tagged raw cycle-count/latency data with guardrail IDs - creating the *appearance* of broad coverage - but none of them ever actually checked that data against `goals/performance/catalogue.tsv`'s real numeric thresholds. The adversarial verification pass did that arithmetic and found most of it fails or is unverifiable as reported (T0 cycle counts run 5-140x over several budgets; several latency guardrails are specified in microseconds with no documented QEMU-TSC-frequency assumption to convert cycles into real time; `G05`'s own computed run-to-run CV already exceeds its own target without anyone flagging it; `G18` is measuring the wrong phase). No number was found fabricated - every figure traces to a real command this session actually ran - but "real number" and "closes the guardrail" are not the same claim, and conflating them would have been dishonest.

**The true, corrected count:** of `PERF-D07`'s 23 non-Claim guardrails (`G01`-`G23`), **1 is genuinely closed** (`G11`, steady-state allocations = 0 - categorically true by construction, since the kernel wires in no `#[global_allocator]` at all), **2 are correctly `N/A-debt`** (`G08` microarchitectural counters - no vPMU under QEMU/TCG on this Windows host; `G19` isolation under competing load - no concurrent-load scheduler infrastructure exists), **5 were not attempted** (`G09`, `G15`, `G16`, `G17`, `G23`), and **14 have real Host/T0 evidence gathered but do not close** against their numeric target or their own required analysis (`G01`-`G07`, `G10`, `G12`-`G14`, `G18`, `G20`, `G21`). `G22` was not run at all this session (see below). `G24`/`G25` (Claim-stage) were never attempted and are never claimed closed. Full per-guardrail table: [`REPORT-2026-07-27-01`](../../goals/reports/REPORT-2026-07-27-01.md).

Real, durable value did come out of this session even so: a genuine Tier-0 QEMU fixture and COM1 serial driver now exist where none did before (both permanent, reusable by future Stories), the Host diagnostic harness is real and will keep producing data as `mem.rs` changes, two real bugs were found and fixed during QEMU bring-up (a stack-overflow from monolithic fixture-phase locals, and a `memcmp`/`bcmp` lowering bug in this target's `core`/`compiler_builtins` combination breaking array-equality comparisons), and the SEC/BND mapping section is honestly and conservatively scoped throughout. The gap this handover is correcting is specifically the earlier, inflated *coverage* narrative, not the underlying commands or data.

## G22 (72-hour soak) status

`G22` requires 72 continuous hours of soak evidence (zero hard-deadline misses, zero memory growth, p99 drift <=5%, zero unexpected safe-state transitions) - not something any single session can produce inline. A recurring scheduled job (`CronList` job id `9fb25cc7`, "the recurring G22 (72-hour soak stability) check for TinyOS STORY-P0-03...", firing every 4 hours) already exists in this environment, checked and confirmed present at the start of this session - it was not created new here, it predates this handover. Roughly every 4 hours across the ~72-hour target window it will re-check soak status, giving on the order of 18 checkpoints rather than one single fire at the end. **This is explicitly named as a limitation, not a guarantee:** per the scheduling tool's own documentation, jobs of this kind are session-scoped and best-effort - they live only in the originating session's memory, are not written to disk, do not survive that session ending, a host reboot, or an environment reset, and recurring jobs auto-expire after 7 days regardless. It is very plausible this job does not actually survive to complete a genuine 72-hour window the way a durable CI pipeline would. The real fix is a durable CI job (e.g. a scheduled GitHub Actions/self-hosted runner workflow that survives independently of any one agent session) running the actual 72-hour soak fixture and filing its own Report on completion; that infrastructure does not exist yet and is recommended as a concrete next step, not assumed solved by the best-effort cron job noted here.

Per the hard rule governing this work: `goals/assurance/story-contracts.tsv`'s `STORY-P0-03-01` row is confirmed unchanged at `state=baseline-debt` (verified via direct `grep` before and after this session's changes) - it does not flip to `verified`, both because `G22` has not closed and because, as corrected above, most of the other 22 guardrails have not closed either.

## Files touched this session

- New: `os/src/hal-x86_64/src/serial.rs` (COM1 16550 UART driver, permanent).
- New: `os/src/kernel/src/fixture_pool_bench.rs` (Tier 0 `Pool<T,N>` benchmark fixture).
- Changed: `os/src/hal-x86_64/src/lib.rs` (`pub mod serial`), `os/src/kernel/src/main.rs` (fixture wiring), `os/src/kernel/Cargo.toml` (`fixture-pool-bench` feature), `os/src/xtask/src/main.rs` (`--fixture=pool-bench` match arm), `os/src/kernel/src/mem.rs` (6 new `#[cfg(test)]` diagnostic functions only - no production code changed).
- New: `goals/reports/REPORT-2026-07-27-01.md` (full per-guardrail scorecard).
- Changed: `goals/tests/TEST-P0-03-01-A.md` (Status line, new Report entry).
- Changed: `goals/stories/STORY-P0-03-01.md` (new dated section documenting this work honestly).
- Unchanged (confirmed): `goals/assurance/story-contracts.tsv` (`STORY-P0-03-01` row still `baseline-debt`).
- New: this file and `session/hand-2026-07-27/index.html`.

## Immediate next steps

1. State and justify a QEMU-TCG TSC-frequency assumption (or an explicit "unverifiable, no timebase" label) so the already-collected T0 cycle counts can actually be checked against the microsecond-denominated `PERF-D07` guardrails (`G01`-`G03`, `G12`-`G14`, `G20`, `G21`).
2. Write `G04`'s missing WCET-margin argument from the already-collected occupancy-pattern data.
3. Fix `G18`'s mislabeling in `fixture_pool_bench.rs` onto the correct reuse-specific phase.
4. Build the still-missing harnesses for `G09` (footprint delta), `G15` (throughput), `G16` (sustained burst), `G17` (30-boot cold start), `G23` (spoor overhead A/B).
5. Run a Miri/sanitizer/fuzz pass against `mem.rs` for `SEC-19`'s named evidence gap.
6. Stand up a durable CI job for `G22`'s 72-hour soak rather than relying on this session's best-effort scheduled job.
7. Once the above genuinely closes every applicable guardrail (including a completed `G22`), flip `STORY-P0-03-01`'s `story-contracts.tsv` row to `verified` - not before.
