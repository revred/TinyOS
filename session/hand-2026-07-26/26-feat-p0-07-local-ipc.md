# Handover 26 — `FEAT-P0-07` Local IPC Implemented and Functionally Verified

Follows: [`25-story-p0-05-04-and-txe-packer.md`](25-story-p0-05-04-and-txe-packer.md).

## What landed

1. **`STORY-P0-07-01` — bounded local message channel.** Added `kernel::ipc::Channel<CAP, MSG_LEN>`, a fixed-capacity, no-heap, non-blocking, directional FIFO between exactly two `TaskId`s. Wrong-direction and third-task calls fail with `NotAnEndpoint`; policy denial, full, empty, oversize, FIFO wrap, and payload round-trip are tested. The primitive has no I/O or network dependency and creates no listener.
2. **`STORY-P0-07-02` — shared-memory grant/revoke.** Added `exec::shared_memory`, `AddressSpace::map_page`/`unmap_page`, and `hal_x86_64::paging::unmap_4k`. A grant aliases owner pages into one sharee with no permission escalation; owner-only revoke unmaps them.
3. **Tier-0 fixture.** Added `fixture_shared_memory_main.rs` and `xtask qemu-x86_64 --fixture=shared-memory`, proving grant, rejected non-owner revoke, valid owner revoke, and target page-table behavior.
4. **V&V.** Added `TEST-P0-07-01-A`, `TEST-P0-07-02-A`, `REPORT-2026-07-26-21`, and `REPORT-2026-07-26-22`; Feature and both Stories are functionally Verified.
5. **Assurance state.** Both Stories remain `baseline-debt`, not assurance `verified`. D12/D13 and their mapped security controls have no raw timing, allocation, active-address-space, production-ACI, hostile-load, or isolation evidence.

## Verification

- `cargo test --workspace --lib`: 141/141 pass (`exec` 47, `hal` 4, `hal-x86_64` 30, `kernel` 60 — see "Fixed this session" below for the extra `kernel` test).
- Shared-memory QEMU fixture passes; seven prior QEMU fixtures remain passing per Report 22.
- Workspace fmt and library clippy are clean.
- Crate-size and 16,032-byte kernel image gates pass.

## Fixed this session (found via the threat/Fable-class security review)

A manual threat-model review (independent of, and cross-checked against, `goals/security/current-state-review.md`'s control-by-control audit) walked the SEC-20 "capacity-zero" line item down to a concrete, reproducible bug rather than leaving it as a general note:

- **`SpoorJournal::<0>::append` panicked unconditionally on its first call** — `self.entries[self.next] = ...` indexed a zero-length array (out-of-bounds), before the following `% N` division (also div-by-zero) was even reached. This directly contradicted the module's own doc comment ("`append` never allocates, never blocks, and never panics"). Fixed with an `N == 0` no-op guard at the top of `append` (`os/src/kernel/src/spoor_journal.rs`); new regression test `a_zero_capacity_journal_never_panics_on_append`. Confirmed `kernel::ipc::Channel<0, _>` was already safe by existing check ordering (`send`'s `self.len == CAP` check returns `Full` before ever reaching `% CAP`) — no fix needed there, but worth having verified rather than assumed.
- This was a narrow correctness fix, not a security architecture change — it doesn't move any Story off `baseline-debt`.

## Assurance follow-ups for next session

The functional acceptance criteria are met, but the subsequent security-spine audit (`goals/security/current-state-review.md`) found prerequisites for release assurance. In the audit's own priority order ("Immediate release-blocking technical findings"):

1. **Activate the isolation that is currently only data.** No IDT/fault handling, no per-task `CR3` switch, no full security-context save/restore, no guard pages, no teardown, and the boot identity map is still broad RWX. `AddressSpace` (`exec::address_space`) builds correct page tables but nothing ever loads them — this is the single largest gap: every "process isolation" claim in this codebase is currently unenforceable at runtime.
2. **Make shared-memory grant creation transactional** (`exec::shared_memory::grant`): validate `pages > 0`; roll back partially-mapped pages on a mid-loop `FrameExhausted`; add a generation/epoch field to `SharedGrant` so a stale token can't be confused with an unrelated later grant reusing the same `sharee_virt`; register live grants for task-exit revocation.
3. **Remove `AllowAllPolicy` from anything but test/fixture code.** It's `pub`, ungated, and already wired into a non-test fixture binary (`exec::win32_shim`'s `fixture_win32_shim_main.rs`) — a production build today has no structural barrier stopping it from being the *only* policy anyone wires in. Needs a real default-deny policy type before any production call site is added.
4. **Define signed executable metadata before ever executing a TXE's entry point** — content hash, signer, revocation, anti-rollback counter, capability manifest. `xtask pack-txe` (`STORY-P0-08-01`) does zero signing today; it only re-lays-out section offsets.
5. **Make critical spoors non-losable under ring-buffer pressure.** `SpoorJournal` overwrites the oldest entry unconditionally, with no severity/priority concept — an adversary flooding low-severity events could push a genuinely critical one out just as easily as noise.
6. **Keep optional stacks (drivers, TCP/IP, storage, browsers) physically absent with automated evidence**, not just "not yet built" — before any of them land, so SEC-13/SEC-18's "unselected driver has zero footprint" claim has a real CI gate behind it from day one.

None of these are grounds to rewrite the functional Verified results as security proof — every Story implemented this session (and `STORY-P0-05-04`/`FEAT-P0-08`/`FEAT-P0-06` before it) remains explicit `baseline-debt` in `goals/assurance/story-contracts.tsv` until raw evidence closes its mapped `SEC-*`/`Dnn` gates.
