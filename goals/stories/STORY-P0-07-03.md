# STORY-P0-07-03 — `shared_memory::grant` Fails Closed Instead of Panicking

Status: **Verified (Tier 1), 2026-07-29** — assurance state `baseline-debt`; `LE-40` closed, `LE-52` registered. Tier 1 only, and correctly so: this Story removes panics from a path no QEMU fixture drives, and a host test that provokes the panic is sharper evidence than a boot that never reaches it
Feature: [`FEAT-P0-07`](../features/FEAT-P0-07.md)
Introduced in: [`session/hand-2026-07-28/29-next-session-mandate.md`](../../session/hand-2026-07-28/29-next-session-mandate.md), which raised `LE-40`; [`43A`](../../session/hand-2026-07-28/43A-degrade-and-inheritance-compose.md) §7 and [`45A`](../../session/hand-2026-07-28/45A-the-composed-scenario-under-preemption.md) §8 both name it the recommended next work, blocked on nothing

## Description

**`LE-40`.** Its remedy line splits the row in two: *"State the invariant in the function contract
now (minutes); replacing the `.expect` with a fail-closed path is a Story, and it must happen before
any SMP work."* The first half landed with the row. This Story is the second half.

`grant` is a kernel path in a system whose first rule is **safety before security before correctness
before performance**, and whose sixth is **fail-safe over keep-trying**. It already returns
`Result<SharedGrant, SharedMemoryError>` and already rolls back a partial mapping on failure. A panic
inside it is not a stricter failure than an error — it is a strictly worse one, because it destroys
the caller instead of the call, on a path whose entire job is to be transactional.

### The finding that made this bigger than the row described

`LE-40` named one `.expect`, and was careful to say it was **not** the TOCTOU it had originally been
reported as. That analysis holds and is unchanged. But auditing the whole function for the same
*defect class* — arithmetic and assertions that panic rather than fail closed — found a second
instance that is materially worse than the one the row named:

**`grant` computed every page address with unchecked `+` and `*` on caller-chosen values.** A
page-aligned `sharee_virt` of `0xFFFF_FFFF_FFFF_F000` with two legitimately-mapped owner pages
passes the alignment check and passes the kernel-region check, then overflows on the second page:

```text
thread '...a_grant_whose_sharee_range_overflows_the_address_space_is_rejected'
panicked at src\exec\src\shared_memory.rs:228:35: attempt to add with overflow
```

Unlike the `.expect`, **this is reachable today** — single core, no shared page directory, no SMP.
And the release-build behaviour is why this is `SEC-03`/`SEC-18` rather than a tidy-up: with
overflow checks off there is no panic at all, the address wraps silently to `0x0`, and the wrapped
value is *never re-checked against `KERNEL_RESERVED_REGION_END`* — the function is already past that
check by the time it computes the address.

The two findings are one defect in two grammars, so they are fixed and tested together.

## Depends on

`STORY-P0-07-02` (the `grant`/`revoke` implementation this hardens), `STORY-P0-05-02`
(`AddressSpace::translate`/`map_page`, the primitives the grantability verdict is read through).

## Acceptance criteria

1. **Grantability is decided in exactly one place.** *Is this owner page mapped?* and *does it carry
   at least the authority the sharee asked for?* are one verdict about one page, answered by one
   function that both of `grant`'s owner-facing loops call. The mapping loop previously re-read the
   translation, `.expect`ed its presence, and **re-checked its permissions not at all** — so under
   either condition `LE-40` names as invalidating the re-read, the presence half panicked while the
   permission half would have mapped the page anyway. The privilege-escalating half was the
   unguarded one.
2. **The re-read fails closed and rolls back.** A rejected re-read unmaps every page this call has
   already mapped and returns the error, sharing one rollback path with frame exhaustion. *Not
   claimed: that the branch is reachable today.* It is not, for the reason `LE-40` states. It is the
   precondition `LE-40` attaches to any future SMP work, and it is now met for this function.
3. **Address arithmetic is checked, before any page is inspected.** A range whose page addresses do
   not all fit in a 64-bit address space is rejected with a new `SharedMemoryError::RangeOverflow`,
   with nothing mapped — for `owner_virt`, for `sharee_virt`, and for `pages` (which is multiplied by
   `PAGE_SIZE`). Checked once up front rather than per-iteration, so the loops below may keep plain
   arithmetic against an invariant that is now **stated where it is established**, which is precisely
   what `LE-40` faulted the old code for not doing.
4. **The generation counter refuses to wrap or saturate.** It is the entire basis of `StaleGrant`:
   a wrapped counter makes a stale token match a later grant, and a saturated one gives every
   subsequent grant the same generation, with the same effect. An exhausted counter is reported as
   `RegistryExhausted` with the mapping rolled back — the only disposition that keeps the token's
   meaning intact.
5. **`revoke` inherits the range guarantee rather than re-deriving it.** `grant` is `SharedGrant`'s
   only constructor, so a token that exists describes a representable range. Recorded as a documented
   invariant on the type, not as a second runtime check: a check that cannot fail is dead code, and
   dead code on a kernel path reads like a real guard.
6. **The class does not come back.** A test reads this module's own source and asserts that the half
   above `#[cfg(test)]`, comment lines stripped, contains no `.unwrap()`, `.expect(`, `panic!`,
   `unreachable!`, `todo!` or `unimplemented!` — `LE-33`/`LE-35`/`LE-36`/`LE-44`'s pattern of putting
   a machine behind a prose rule. It **fails on the pre-fix source**, naming the line, which is what
   makes it an instrument rather than a decoration.

## Named debt this Story leaves open

- **`LE-52` is registered, not closed.** The §6 gate covers one module. Every other non-test path in
  `kernel`/`exec` is ungated, and no lint in this workspace enforces it. Generalising the gate is a
  sweep with its own scope decision, not a line in this Story.
- **Implicit panics are not covered.** The gate catches explicit constructs. Slice indexing, division
  and arithmetic still panic implicitly; criterion 3 removes the instances this function had, and
  proves nothing about the next one. Named in `TEST-P0-07-03-A` §5 and in `LE-52`.
- **SMP is not made safe.** One of its preconditions is removed, for one function. `LE-40`'s
  ordering constraint is satisfied here and nowhere else.
- **No performance guardrail closes.** `D13` is selected because shared-memory grant is that domain's
  subject, not because this Story measures anything. No `guardrail-evidence.tsv` row, no
  `TOS64-MEAS/2` envelope.
- **No Tier 0 evidence.** Deliberate: no fixture drives `grant`, and the claim is the *absence* of a
  panic. A host test that provokes the panic and watches it become an error is stronger evidence than
  a boot that never reaches the code.

## Tests

[`TEST-P0-07-03-A`](../tests/TEST-P0-07-03-A.md) — written before implementation, per the TDD
mandate. Eight new host tests; four of them failed on the pre-fix source, one by panicking.

## Reports

- [`REPORT-2026-07-29-01`](../reports/REPORT-2026-07-29-01.md)
