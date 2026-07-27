# TINYCMD: DOS + POSIX CLI Compatibility — MVP Spec

Status: **draft / companion to Roadmap Phase 2 (Shell & UX)**

## Purpose

TinyOS's shell, `TINYCMD`, needs to feel native to two very different populations of muscle memory: operators who think in `DIR`/`COPY`/`DEL` and DOS-style `/switches`, and operators who think in `ls`/`cp`/`rm` and POSIX-style `-flags`, pipes, and redirection. Rather than implementing each command twice, `TINYCMD` is built as **one canonical command core with two syntax front-ends** that both compile down to the same verbs — consistent with Design Pillar 2 (UX/UI strictly separated from control): whichever syntax an operator types, the same ACI-gated backend executes it, with the same audit trail.

Reference note: the [`MsDOS`](../MsDOS) submodule (Microsoft's officially released MS-DOS source, `v4.0/src/CMD`) was reviewed for *which* commands existed and their general behavior, not for implementation — TinyOS's shell is an original, from-scratch Rust implementation per [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#language-policy), not a port.

## Architecture

```text
"DIR /S *.TXT"          "ls -R *.txt"
      │                       │
      ▼                       ▼
┌────────────┐         ┌───────────────┐
│ DOS parser │         │ POSIX parser  │    ← two thin syntax front-ends
└──────┬─────┘         └───────┬───────┘
       │                       │
       └───────────┬───────────┘
                    ▼
         ┌───────────────────────┐
         │   Canonical Verb      │   list / copy / move / delete / mkdir / rmdir /
         │   Core (TinyCmd)      │   view / find-text / sort-stream / page /
         │                       │   tree / env-get / env-set / task-list / task-kill / ...
         └───────────┬───────────┘
                     ▼
         ┌───────────────────────┐
         │   Agent Command       │   ← same ACI gate as HBP/WCI/agent callers
         │   Interface (ACI)     │
         └───────────────────────┘
```

- Both front-ends emit the same internal, typed command struct (verb + normalized arguments). Everything downstream of that point — capability check, audit log, execution — is syntax-agnostic.
- Piping (`|`) and redirection (`>`, `>>`, `<`) use the same symbols in both worlds and share one implementation in the core.
- Front-end selection is per-session, not per-install: an operator can pick `dos` or `posix` mode (or the shell can auto-detect from the first token's casing/flag style), so the same device serves both audiences without a rebuild.

## MVP verb set

The MVP intentionally covers the commands that account for the large majority of real interactive and scripted use, not the full historical DOS command set or full GNU coreutils. Each row is one canonical verb with both syntax bindings.

| Canonical verb | DOS binding | POSIX binding | Notes |
|---|---|---|---|
| `list` | `DIR` | `ls` | `/S`↔`-R` (recursive), `/W`↔ column view, `/P`↔ paged |
| `change-dir` | `CD`, `CHDIR` | `cd` | No-arg form prints current dir in both |
| `print-cwd` | `CD` (no arg) | `pwd` | |
| `copy` | `COPY`, `XCOPY` | `cp` | `/S`↔`-r` for recursive tree copy |
| `move` / `rename` | `MOVE`, `REN` | `mv` | Same verb; destination-is-dir vs rename disambiguated identically to POSIX `mv` |
| `delete` | `DEL`, `ERASE` | `rm` | No POSIX-style force-recursive by default; `/S`↔`-r` still requires confirmation policy (see Safety) |
| `make-dir` | `MD`, `MKDIR` | `mkdir` | |
| `remove-dir` | `RD`, `RMDIR` | `rmdir` | Non-empty removal requires explicit recursive flag in both |
| `view-file` | `TYPE` | `cat` | MVP: no `cat`-style multi-file concatenation-with-separators; just sequential dump |
| `find-text` | `FIND` | `grep` | MVP: literal substring + basic wildcard, not full regex |
| `sort-stream` | `SORT` | `sort` | |
| `page` | `MORE` | `more` (not full `less`) | |
| `tree-view` | `TREE` | `tree` | |
| `attrib-view` | `ATTRIB` | `ls -l` (permission column) | Maps DOS file attributes (R/H/S/A) to a capability-scoped view, not a POSIX multi-user permission model — see Non-Goals |
| `env-get`/`env-set` | `SET`, `PATH` | `env`, `export`, `echo $VAR` | `%VAR%` and `$VAR` expansion both supported by the parser layer |
| `echo` | `ECHO` | `echo` | |
| `clear-screen` | `CLS` | `clear` | |
| `version-info` | `VER` | `uname -a` (subset) | |
| `volume-info` | `VOL` | `df` | Reports TinyOS storage/partition info, not POSIX mount table semantics |
| `mem-info` | `MEM` | `free` | Reports RT memory pool usage per [Design Pillar 1](../README.md#1-a-real-multitasking-rtos-core), not general-purpose heap stats |
| `task-list` | (new — no strong DOS analog) | `ps` | Backed by the ACI-visible RT task table; DOS side gets a `TASKMGR`-style command by convention |
| `task-kill` | (new) | `kill` | Gated by ACI capability scope; never allowed to kill an RT-critical task without `supervisor` scope |

## Non-goals for MVP

- **No text editor** (`EDLIN`/`vi`/`nano`-class). Deferred; file editing happens off-device for now.
- **No multi-user POSIX permission model.** `chmod`/`chown` are not meaningfully portable to a device with TinyOS's single-capability-registry security model (see [ACI](../README.md#5-llm-as-a-supervised-operator-not-a-root-user)); `attrib-view` exposes DOS-style file attributes and ACI capability scope instead of a Unix UID/GID/mode triad.
- **No full regex** in `find-text`/`grep` at MVP — literal and basic wildcard matching only; full regex is a post-MVP addition.
- **No networking commands** (`curl`, `wget`, `ping`-beyond-diagnostic) — connectivity is governed by [HBP](../README.md#inter-os-communication-the-host-bridge-protocol-hbp)/[WCI](../README.md#remote-control-the-wireless-command-interface-wci), not ad hoc shell tools, to avoid an unaudited path around the ACI.
- **No package manager.** Out of scope entirely for an RTOS shell.
- **Batch scripting** (`.TCB`, per the [DOS Inheritance](../README.md#the-dos-inheritance) section) stays DOS-flavored (`IF`/`FOR`/`GOTO`/`CALL`/`%1`) for MVP; a POSIX-shell-style scripting mode is a later addition once the DOS-flavored path is proven, not built in parallel from day one.

## Safety & security notes

- Every verb — regardless of which syntax front-end produced it — passes through the ACI policy engine exactly like an HBP/WCI/agent-originated command. There is no "local shell" privilege shortcut.
- Destructive verbs (`delete`, `remove-dir` with recursive flag) require explicit confirmation by default in interactive sessions; scripted/batch invocation requires an explicit non-interactive flag, so a script can't silently inherit an interactive default.
- `task-kill` against an RT-critical task requires `supervisor` capability scope, never `operator`, consistent with the co-bot/CNC authority model already specified for WCI.
- Per [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#test-driven-development-mandatory), every verb ships with tests under **both** syntax front-ends (a DOS-syntax test and a POSIX-syntax test exercising the same canonical verb), plus adversarial tests for path traversal (`../../`, absolute-path escapes) and argument-injection attempts through either parser.

## Phased plan

1. **Core verb engine** — canonical verb types, execution against the filesystem/task-table backend, ACI integration. No parsers yet; internal API only, fully unit-tested (TDD).
2. **POSIX front-end** — flags, pipes, redirection, `$VAR` expansion, globbing. Chosen first because piping/redirection semantics are easiest to validate against the more regular POSIX grammar.
3. **DOS front-end** — `/switches`, `%VAR%` expansion, DOS-style path separators accepted alongside POSIX-style ones.
4. **Session mode selection** — per-session `dos`/`posix` mode switch, plus auto-detect heuristic.
5. **Golden-file acceptance suite** — for each MVP verb, one fixture run through both front-ends, asserting equivalent underlying action and equivalent (front-end-appropriate) output formatting.

## Status

This document defines the Phase 2 shell scope. It will be revised as the ACI capability registry (Roadmap Phase 5) solidifies, since every verb here is gated by it.
