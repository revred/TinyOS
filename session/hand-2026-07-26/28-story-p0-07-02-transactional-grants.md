# Handover 28 — `STORY-P0-07-02` Hardened: Transactional, Generation-Safe Shared-Memory Grants

Follows: [`27-assurance-and-security-spine.md`](27-assurance-and-security-spine.md).

## What this picks up

Handover 27's security-spine audit (`goals/security/current-state-review.md`) named a specific, concrete gap in `exec::shared_memory::grant`/`revoke` (`STORY-P0-07-02`) and listed it first among its "immediate release-blocking technical findings": make grant creation transactional (`pages > 0` validated, partial mappings rolled back on mid-loop `FrameExhausted`), add a generation/epoch concept so a stale token can't be confused with an unrelated later grant reusing the same `sharee_virt`, and register live grants for task-exit revocation.

This session closes the first two in full and the third as far as it can honestly go.

## What changed

`os/src/exec/src/shared_memory.rs`:

1. **`grant` rejects `pages: 0`** outright (`SharedMemoryError::ZeroPages`) — not a region, nothing to share.
2. **`grant`'s mapping loop is now transactional.** If a later page's mapping fails partway through (e.g. `FrameExhausted` because the sharee's frame pool runs out), every page already mapped by that same call is unmapped again before the error returns — a failed `grant` never leaves a partial, half-usable region live in the sharee's address space. A new host test (`grant_rolls_back_partial_mapping_on_frame_exhaustion`) forces this deliberately: `sharee_page_0` is the last 4KiB page of one 2MiB page-table block and `sharee_page_1` the first page of the next, so a 3-frame sharee pool lets page 0's mapping succeed (consuming a fresh PDPT+PD+PT) and starves page 1's (which needs one more PT).
3. **New `GrantRegistry<CAPACITY>`** — a fixed-capacity, no-heap table of live grants. `grant` stamps every successful grant with a registry-assigned, monotonically increasing generation and records `(sharee_virt, generation)`; `revoke` looks up the record for the token's `sharee_virt` and rejects with the new `SharedMemoryError::StaleGrant` if a *different* generation now occupies it — the actual fix for "a stale token can't be confused with an unrelated later grant reusing the same `sharee_virt`." A full registry also rolls back the mapping (`SharedMemoryError::RegistryExhausted`), for the same reason a `FrameExhausted` mid-loop failure does: a page mapped with no registry record would be unrevokable.
4. **New `GrantRequest`** — `owner_virt`/`sharee_virt`/`pages`/`sharee_permissions` bundled into one plain-data struct. Adding the `registry` parameter to `grant` would have pushed it to 8 arguments, tripping `clippy::too_many_arguments`; this brought it back to 5 by grouping the region-shaped data, not by suppressing the lint.
5. **`fixture_shared_memory_main.rs`** updated to thread a static `GrantRegistry<4>` through its existing grant/revoke calls — its own asserted behavior (well-formed grant, non-owner revoke rejection, owner revoke) is unchanged.

Four new host tests in `shared_memory::tests`: `granting_zero_pages_is_rejected`, `grant_rolls_back_partial_mapping_on_frame_exhaustion`, `grant_rolls_back_the_mapping_when_the_registry_is_full`, `revoke_rejects_a_stale_token_whose_address_was_regranted`. Every pre-existing test was updated in place to construct a `GrantRegistry` and pass a `GrantRequest`, not rewritten in substance.

## Deliberately not done: task-exit revocation

The third item on the follow-up list — "register live grants for task-exit revocation" — is **not implemented**. `kernel::sched` has no `exit`/`terminate` primitive anywhere in this codebase; there is no task-exit event to hook a revocation into yet. Wiring `GrantRegistry` lookups into a teardown path that doesn't exist would be speculative, untested behavior, not a real capability — the same "don't add code with no real caller" discipline `STORY-P0-03-02`'s own handover already established for capacity constants. This is recorded as an explicit open item, not silently dropped: it belongs with the larger "activate the isolation that is currently only data" work Handover 27 already named as the single largest remaining gap (no task-exit teardown is one symptom of that same missing subsystem, alongside no IDT, no per-task `CR3` switch, and no guard pages).

## Verification

- `cargo test -p exec --lib`: 51/51 passing (up from 47).
- `cargo test --workspace --lib`: 145/145 passing (`exec` 51, `hal` 4, `hal-x86_64` 30, `kernel` 60).
- `cargo build -p exec --bin shared-memory-fixture --target targets/x86_64-tinyos.json -Z build-std=core,alloc,compiler_builtins -Z build-std-features=compiler-builtins-mem -Z json-target-spec`: compiles clean against the real target. **No QEMU binary is available in this session's environment**, so the fixture was not re-run under QEMU this time — a real gap, not glossed over; a future session with QEMU available should re-run `cargo run -p xtask -- qemu-x86_64 --fixture=shared-memory` to reconfirm it.
- `cargo fmt --check` (workspace): clean.
- `cargo clippy --workspace --lib -- -D warnings`: clean.
- `cargo run -p xtask -- check-crate-sizes`: `exec` 1,999 lines (from 1,840), far under the 20,000-line ceiling.
- `cargo run -p xtask -- check-image-size`: kernel release image unchanged, 16,032 bytes.
- `cargo run -p xtask -- check-assurance-spine`: 25 Stories, 22 Tests, 24 Reports, 20 security controls, 1,025 selected Story/performance contracts — `STORY-P0-07-02` remains `baseline-debt` (this closes a functional acceptance criterion, not assurance evidence).

New `STORY-P0-07-02` acceptance criterion 5 documents this addition; `TEST-P0-07-02-A` and new `REPORT-2026-07-26-24` cover it.

## What this does not claim

- `STORY-P0-07-02` is still `baseline-debt`, not assurance `verified` — no raw timing, allocation, active-address-space, or hostile-load evidence exists for D13/SEC-03/SEC-04/SEC-18/SEC-20.
- Task-exit revocation remains open, blocked on a task-exit mechanism that doesn't exist anywhere in this scheduler yet (see above).
- No production call site in `kernel` calls `grant`/`revoke` — unchanged from Handover 26.
- The QEMU fixture's own behavior was not re-observed passing this session (build-only verification against the real target); see the verification section above.

## Immediate next steps

Handover 27's own priority-ordered list stands, unchanged except for its first item now being materially smaller in scope:

1. Activate the isolation that is currently only data — IDT/fault handling, per-task `CR3` switch, full security-context save/restore, guard pages, task-exit teardown, and removing the broad boot RWX identity map. This remains the single largest gap, and is also the prerequisite for closing the task-exit-revocation item this session deferred.
2. Remove `AllowAllPolicy` from anything but test/fixture code before any production call site is added.
3. Define signed executable metadata before ever executing a TXE's entry point.
4. Make critical spoors non-losable under ring-buffer pressure.
5. Keep optional stacks physically absent with automated evidence, not just "not yet built."
