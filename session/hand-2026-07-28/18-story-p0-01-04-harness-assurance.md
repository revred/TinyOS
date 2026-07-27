# Handover 18 — `STORY-P0-01-04`: nine fixtures nobody was running

Written at the close of 2026-07-28, after the agenda's item A4/A5 were taken together. `STORY-P0-01-04` is Verified. **`FEAT-P0-01`'s exit had been claimed on weaker evidence than it read as**, and this Story is partly the record of finding that out.

## The finding

**Nine of the twenty-three Tier 0 fixtures existed, compiled, passed — and no CI step ran any of them.**

```text
context-switch, idt-apic-timer, idt-apic-unrouted, pci-enumeration,
address-space, win32-shim, blue-sharc, blue-sharc-broken, shared-memory
```

Every one is named by an owning Test document that claims Tier 0 evidence for it. That evidence was last produced on somebody's development machine, at an unknown commit, and nothing had re-established it since. All nine pass when run, so this is a gap in **evidence**, not in behaviour — but "it passed once, locally" is exactly the standard `LE-07` was raised about.

It went unnoticed because the existing drift guard checked **CI → table** (a step naming a fixture the table lacks). **Table → CI was unguarded**, and that is the dangerous direction: a fixture that exists and is never run is an unverified fixture that looks verified. Both directions are now guarded by host tests.

**The count took two corrections and both would have overstated it.** The first version reported eleven, including `measure` (which CI runs as a bare `measure --runs=1` subcommand, not by `--fixture=` name) and `dispatch-measure` (the *same binary* CI runs as `--fixture=dispatch` — `--fixture=` is overloaded across two namespaces with different spellings for one fixture). Coverage is now credited by **build target** `(package, binary, feature)`, which removes the special case rather than hard-coding it. A third, duller correction: the workflow is checked out CRLF, so anchoring a scan on `"…qemu-x86_64\n"` matched nothing and reported every default-invoked fixture as unrun.

## Why the exit-code hole survived three mandates

It was recorded as *"the CI step should grep the serial capture, per `wcet-trip`"* — which reads like a CI edit, and is why it kept being deferred as cheap. Running the two fixtures shows why nobody finished it:

```text
$ xtask qemu-x86_64 --fixture=broken-boot --serial-capture=bb.log   # exit 1
$ cat bb.log
                <-- empty
```

**There was nothing to grep.** `broken-boot` panics and the panic handler was a bare `exit_qemu(Failure)`; `idt-apic-unrouted` reaches `unhandled_interrupt_handler`, likewise bare.

So the harness gap was hiding a system defect: **a TinyOS kernel that panicked died in complete silence.** On Tier 0 that costs an assertion. On a board — where there is no `isa-debug-exit` code at all — the serial line is the *only* diagnostic there would be, which makes this directly relevant to Handover 17's bring-up plan.

All eleven `#[panic_handler]`s in the workspace were byte-identical bare `exit_qemu` calls, so rather than copy a sentinel eleven times they route through one shared `hal_x86_64::qemu_exit::panic_report`:

```text
TINYOS-PANIC/1 message=fixture-broken-boot: deliberate panic for TEST-P0-01-03-A file=src\kernel\src\main.rs line=141
TINYOS-UNROUTED/1 fail_closed=true
```

## Design decisions that should not be re-litigated

- **The message is emitted before the location.** An assertion keyed on a line number breaks the next time anything above it moves, so `broken-boot`'s CI step greps its own deliberate panic message instead.
- **Ordering is fail-closed.** The sentinel is written *before* the exit port is touched, because the exit port stops the machine and anything after it is not evidence. Every write is `let _ =`: a UART that will not accept bytes must not be able to turn a panic into a hang.
- **The unrouted sentinel does not name its vector** (`LE-25`). One shared stub serves every unrouted vector and receives no vector argument; naming it needs per-vector trampolines, which is a larger change than this Story's charge. The line proves containment was reached, not which vector reached it.
- **`TEST-P0-01-04-A` clause 6 is retrospective and says so.** The `2da1ccd` tooling already had 25 unit tests covering every violation case — register gaps, closure without evidence, out-of-vocabulary states, `Complete` not matching `Completely`, `Functionally Verified` not truncating to `Verified`. What it lacked was the assurance artifact, which is what made shipping it a bypass of rules 3 and 8 rather than a testing gap. Clauses 1–5 are genuinely Red-first. **Back-filling a document over green code is honest only if it says that is what it is doing**, and blurring that is how the discipline erodes.

## Loose-ends delta

**New:** `LE-25` — the unrouted sentinel cannot name its vector.

**Reinforced, not closed:** `LE-07`. The new guard catches a fixture nobody runs. It does not catch a fixture that runs and whose result nobody reads.

## State of the tree

Committed and merged; `main` is at `d0a2a60`. **Not yet pushed at the time of writing.**

`git add -A` swept Handover 17 (the Raspberry Pi 5 bring-up plan, written concurrently and not part of this Story) into commit `49acf55`. It is a decomposition proposal, nothing in it is implemented, and it is unrelated to this Story's diff. This handover took slot 18 as a result.

Verification at the close, all green:

- **437 host tests** (435 before; 2 added — the two new registry cross-checks).
- `cargo fmt --all -- --check`, `cargo clippy --workspace --lib --tests -- -D warnings`.
- Per-binary target clippy for `kernel`, `exec` and `os`.
- `check-assurance-spine` (22 Features / **50** Stories / **37** Tests / **44** Reports / **25** loose ends, 15 open).
- **The full Tier 0 sweep, all 24 invocations**, each matching its documented pass condition — necessary rather than optional this time, since nine of them are newly wired into CI and pushing a red workflow was the failure mode to avoid.

## Standing constraints — unchanged

- **TDD.** Test document when a Story starts. Clauses 1–5 here were Red-first; clause 6 is labelled retrospective.
- **Tier 0 is not hardware evidence.** `LE-09` open; see Handover 17.
- **Never call TinyOS a hobby OS.** It targets data-centre, local-AI, UAV, medical, edge and consumer deployment.
- **No assurance state may be claimed beyond its evidence** — which is exactly what this Story found `FEAT-P0-01` had been doing.
