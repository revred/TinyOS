# Handover 03 — STORY-P1-03-02 Hardening Review: Nine Defects Found Before Implementation Started

Follows: [`02-first-real-task-integration-proposal.md`](02-first-real-task-integration-proposal.md). A pre-implementation design review of `STORY-P1-03-02`'s finalized acceptance criteria, done the way a reviewer reads a graduate engineer's proposal: assume the intent is right, then check every claim against what the code actually is. Nine defects survived that check — three of them would have made the Story's central claim unimplementable or vacuously true as written. Each is recorded here with its resolution; `STORY-P1-03-02.md`'s acceptance criteria are rewritten against these before any code.

## D1 — "An out-of-allowlist Win32 call from inside the task raises a real fault" conflates three mechanisms, and none of them faults

The criterion assumed a loaded task can *make* a Win32 call through the shim and that an out-of-allowlist one *faults*. Neither is true of the code:

- `exec::win32_shim` is a **Rust-level API** (`resolve`, `check_imports`, `write_file`, ...) — there is no IAT patching anywhere in the loader (`STORY-P0-05-04` explicitly declined to jump into the image, and nothing writes function pointers into the loaded image's import address table). Code *inside* the image has no path to the shim at all.
- An out-of-allowlist import is refused at **load time** by `check_imports` — `blue-sharc.exe`'s real 205-import surface is already proven rejected (`blue-sharc-fixture`). It never gets to "call from inside."
- An in-allowlist but policy-denied call returns `Err(ShimError::PolicyDenied)` — an error return, not a CPU fault, so `kernel::fault` never sees it.

**Resolution — prove both layers, each with the mechanism it actually has.** Layer 1: the load-time gate's refusal of the real import surface becomes a *production* audit event (spoored) on the integration path. Layer 2: runtime containment is proven by deliberately running the image anyway under an explicit, documented override policy (defense in depth: even a task that somehow got past the gate is contained) — the task's `Context` entry point is the image's own real entry (`0x1_4000_0000 + 0x71fe00`, RX-mapped). Real MSVC CRT startup code executes; within a handful of instructions it calls through its **unpatched** IAT — unpatched precisely because no capability was ever granted — and the indirect call lands on a low address that, under this Story's W^X kernel mappings, is non-executable data. That raises a real `#PF` on instruction fetch: nothing hand-placed, deterministic for a fixed binary, and the exact defect shape of an ungoverned real workload reaching for authority it doesn't hold. **Named risk:** what the entry code does before its first IAT call is an empirical question; if it proves non-deterministic or non-terminating under QEMU, the documented fallback trigger is the scheduled task writing to its own RX `.text` page (still a real W^X fault, minimally staged). The fixture run decides which claim the Report files.

## D2 — "The real boot path loads blue-sharc.exe" is structurally impossible in `kernel`'s own binary

`exec` depends on `kernel`; `kernel`'s binary cannot link `exec` back without a cycle (the same reason `exec-fixture` exists at all, per `hal_x86_64::boot`'s doc comment). **Resolution:** the integration binary *reproduces the real boot path* — the same `hal_x86_64::acpi::discover_topology` and `hal_x86_64::pci::enumerate_bus_zero` calls with the same success gates `kernel_main` applies — then continues into task loading. Unifying the two binaries is a named follow-on for when a dedicated top-level `os` binary crate exists, not silently claimed now.

## D3 — "No mapping anywhere in the running system is W^X" silently requires retiring the boot CR3

The single largest W^X violation in the running system is the kernel's own boot identity map: `boot.rs` builds 1GiB of RWX 2MiB huge pages and everything has run on it since `STORY-P0-01-01`. An audit of task trees alone would leave the claim false where it matters most. **Resolution:** this Story builds a W^X-correct **kernel tree** (text RX, rodata RO-NX, everything else RW-NX, from linker-provided section boundary symbols added to `x86_64-tinyos.ld`) and the supervisor itself switches onto it after bring-up; the boot RWX map remains only as the bring-up trampoline, exactly as every production kernel's early-boot map does. Ordering consequence: ACPI/PCI discovery reads firmware tables *outside* the kernel image's own extent, so discovery runs **before** the switch, on the boot map — the audit claim is scoped to "after memory-protection bring-up completes," stated in the criteria rather than discovered in a triple fault.

## D4 — Nothing ever enables the hardware bits that make W^X enforceable

At CPL 0, writes to read-only pages are **allowed** unless `CR0.WP` is set, and PTE bit 63 (NX) is only *valid* when `EFER.NXE` is set. Neither is set anywhere in this codebase. As written, acceptance criterion A1 either passes vacuously (writes to RX pages silently succeed) or fails wrongly (NX bits as reserved-bit faults). **Resolution:** a new `hal_x86_64::paging::enable_nx_and_wp` bring-up step, and A1 explicitly requires it — both W^X directions are proven *with the enforcement bits on*, which is the only configuration under which the proof means anything.

## D5 — The writable-alias hole: per-entry W^X audit passes while W^X is defeated

The loader copies image bytes into caller-supplied `staging`, and the task view maps those staging frames RX. But staging lives in kernel memory, which the kernel view maps RW-NX — so every "immutable" executable page has a live writable alias, and a per-entry audit ("no entry is W and X") passes anyway. **Resolution:** *sealing* — the word already in this Story's title, made concrete. After `AddressSpace::create`, the kernel view of every frame the task maps non-writable is re-protected to RO-NX (`paging::protect_4k`, new), and the audit gains an alias clause: no frame mapped executable anywhere is mapped writable anywhere. Teardown unseals before wiping (the wipe writes through the identity view, which `CR0.WP` would otherwise fault).

## D6 — "Shared kernel mappings" must share at PD granularity, and the criteria didn't say so

The obvious sharing unit — one shared top-level entry — is wrong here: the image base `0x1_4000_0000` (5GiB) and kernel low memory both live under **PML4 slot 0**, so sharing the PML4 entry (or a whole PDPT) would share the image across spaces and break the isolation `STORY-P1-03-01` just proved. **Resolution:** the shared unit is two **page directories** — one for kernel low memory, one for the local-APIC MMIO page at `0xFEE00000` (PDPT slot 3, needed so the real boot path's armed timer keeps working under every space) — installed via a new `paging::install_shared_pd`; each space keeps its own PML4 and PDPT. Sharing is *proven*, not asserted: the fixture reads the PD address back through each tree and confirms both trees resolve to the same physical directory.

## D7 — "Host-tested for both arms" cannot be done for the CR3 arm

`context::switch_address_space` is `cfg`-gated off on Windows hosts, and `mov cr3` cannot execute in user space on any host OS. **Resolution:** the *decision* is factored into a pure, host-tested `dispatch::switch_plan` (`None → Plain`, `Some(cr3) → InstallAddressSpace(cr3)`); `run_once` is untouched (its existing tests are the no-regression guard for the `None` arm), and the new `run_once_in_space` consumes the plan, with the hardware arm proven under Tier 0 where a `mov cr3` can actually retire.

## D8 — The teardown-then-probe criterion is under-specified in three ways that decide the design

(a) A stale-mapping probe needs a CR3 that is still *loadable* — so teardown revokes the image mappings and wipes the frames but **keeps the shared kernel PDs linked**, otherwise the probe faults on instruction fetch of the probe code itself and proves nothing. (b) The probe must run **as a task**: a kernel-context `#PF` is `Disposition::HaltSystem` by policy, so a supervisor-context probe would end the fixture, not prove containment. (c) "A reused frame is provably wiped" means the **staging bytes** (the task's actual data/code frames); page-table frames stay allocated in the pool until the space is fully dropped and are documented as such, not silently counted as wiped. Generation ordering per `PD-13`: revoke, wipe, then advance — with the advance observable before any reuse.

## D9 — "Spoor's first production call site" needed a definition of *production*

Everything real currently lives in fixture binaries, so the phrase was satisfiable by weaseling. **Resolution, two sites:** (1) the **shipping kernel binary's own** `tinyos_fault_entry` in `main.rs` gains a static `SpoorJournal` (capacity from a new `kernel::capacities::SPOOR_JOURNAL_CAPACITY` — the constant four Reports said must wait for a real consumer, which now exists) and appends the audit pair it already computes and discards; (2) the integration supervisor journals the load-refusal, each dispatch round, and the contained fault, and reports the journal length over serial as part of its verdict.

## What stands unchanged from the 2026-07-28 decisions

Folding into `STORY-P1-03-02` (not a new Story); the capability boundary as the adversarial trigger (now honestly split across its two real layers, per D1); ACPI/PCI discovery untouched (now with the D3 ordering made explicit). The Story remains the largest single unit of work in the project; the mitigation is the same discipline as ever — Test document first, host tests before fixtures, one fixture proven before the next.
