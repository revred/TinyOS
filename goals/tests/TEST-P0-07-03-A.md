# TEST-P0-07-03-A — `exec::shared_memory::grant` Fails Closed On Every Path It Can Take

Status: **Verified (Tier 1)** — specification written before implementation, per the TDD mandate
Story: [`STORY-P0-07-03`](../stories/STORY-P0-07-03.md)
Tier: Tier 1 host tests in `os/src/exec/src/shared_memory.rs`. No Tier 0 fixture — this Story removes panics from a path no fixture drives, and a QEMU run cannot observe the absence of a panic more sharply than a host test that provokes it can
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D13`
Security controls: `SEC-03`, `SEC-04`, `SEC-18`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-04`, `BND-08`, `BND-09`, `BND-14`, `BND-15`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-03`, `PD-05`, `PD-06`, `PD-08`, `PD-09`, `PD-13`, `PD-14`
Code admission gates: `RCG-08`, `RCG-09`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`LE-40`, whose remedy line splits the row in two: *"State the invariant in the function contract
now (minutes); replacing the `.expect` with a fail-closed path is a Story, and it must happen
before any SMP work."* The first half is already in the source. This is the second half.

`grant` is a kernel path in a system whose first rule is **fail-safe over keep-trying**. It already
returns `Result<SharedGrant, SharedMemoryError>` and already rolls back a partial mapping. A panic
in that function is not a stricter failure than an error — it is a *worse* one, because it takes
down the caller instead of the call.

**The finding that made this larger than the row described.** `LE-40` named one `.expect`. Auditing
the whole function for the same defect class found a second, materially worse instance: `grant`
computes every page address with unchecked `+` and `*` on **caller-chosen** values. A `sharee_virt`
near the top of the address space overflows on the second page — a debug-build panic, and in a
release build a silent wrap to a low address that the kernel-region check has already been passed.
That is reachable today, from a legitimate owner region, with no SMP and no shared page directory.

The two findings are the same defect in two grammars, so they are fixed and tested together.

## Specification

### 1. Grantability is decided in exactly one place

**Given** the two questions `grant` asks of an owner page — *is it mapped?* and *does it carry at
least the authority the sharee is asking for?* —
**then** both are answered by a single function, and both of `grant`'s loops call it.

This is the substance of the fix, not a tidy-up. The mapping loop previously re-read the owner's
translation and `.expect`ed it, **checking permissions not at all**: the first loop's permission
verdict was carried forward across the re-read on the assumption that the re-read returns an
identical page. Under the two conditions `LE-40` names as silently invalidating (SMP; shared
page-table structure) that assumption fails *for permissions* exactly as it fails for presence —
and the presence half panicked while the permission half would have **mapped the page anyway**.
A privilege-escalating mapping is the worse of the two outcomes, and it was the unguarded one.

### 2. The re-read fails closed, and rolls back

**Given** a mapping-loop re-read that does not satisfy §1,
**then** `grant` unmaps every page it has already mapped in this same call and returns
[`SharedMemoryError::RegionNotOwned`] or `PermissionsExceedOwner` — never panics, never leaves a
partial mapping.

**Not claimed: that this branch is reachable today.** It is not, and `LE-40` says why — `owner_space`
is held under a shared borrow on a single core. It is defence in depth on a path where the cost of
being wrong is a kernel panic, and it is the precondition `LE-40` attaches to any future SMP work.
Its *rollback* shares one code path with the frame-exhaustion rollback, which is reachable and is
covered by an existing test.

### 3. Address arithmetic is checked, on caller-chosen values

**Given** a `GrantRequest` whose `owner_virt`, `sharee_virt` and `pages` describe a range that does
not fit in a 64-bit address space,
**then** `grant` returns a `SharedMemoryError` and maps nothing.

**And** this is provoked, not asserted in the abstract: a request with two legitimately-mapped owner
pages and a page-aligned `sharee_virt` of `0xFFFF_FFFF_FFFF_F000` **panics before the fix** — the
sharee-range loop reaches `sharee_virt + 1 * PAGE_SIZE` and overflows. It is above
`KERNEL_RESERVED_REGION_END`, so the kernel-collision check passes it; it is page-aligned, so the
alignment check passes it. Nothing else stands between a caller and the panic.

**And** the release-build behaviour is the reason this is `SEC-03`/`SEC-18` and not a tidy-up: with
overflow checks off, the wrap is silent and lands the computed address at **0x0000_0000_0000_0000** —
inside the kernel-reserved region the function has already finished checking. The check is not
re-run, because the loop is past it.

### 4. `revoke` inherits the guarantee rather than re-deriving it

**Given** a [`SharedGrant`], which no code outside this module can construct,
**then** `revoke` may walk its range with plain arithmetic, because §3 established at issue time
that the range is representable.

This is asserted as a *documented* invariant on the type, not as a second runtime check. A check
that cannot fail is dead code, and dead code on a kernel path is a maintenance liability that reads
like a real guard.

### 5. The class does not come back

**Given** this module's non-test source,
**then** it contains no `.unwrap()`, `.expect(`, `panic!`, `unreachable!`, `todo!` or
`unimplemented!`.

Gated by a test that reads the module's own text and scans the half above `#[cfg(test)]`, in the
spirit of `LE-33`/`LE-35`/`LE-36`/`LE-44` — a prose rule with a machine behind it. This test fails
on the pre-fix source, which is what makes it an instrument rather than a decoration.

**What it does not claim:** absence of *implicit* panics — slice indexing, unchecked arithmetic,
division. §3 removes the arithmetic instances this module actually had; the gate does not prove a
future one could not be introduced, and no lint in this workspace does either.

## What this test explicitly does not establish

- **That SMP is safe.** It removes one of SMP's preconditions. `LE-40`'s "before any SMP work"
  ordering constraint is satisfied for this function and for no other.
- **Any timing guardrail.** `D13` is selected because shared-memory grant is this domain's subject,
  not because anything here is measured. No `guardrail-evidence.tsv` row, no `TINYOS-MEAS/2`.
- **That the rest of the kernel fails closed.** The gate in §5 covers one module. Registered as
  `LE-52` rather than silently generalised.

## Reports

- [`REPORT-2026-07-29-01`](../reports/REPORT-2026-07-29-01.md)
