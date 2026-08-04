# CLAUDE.md — pointer to `agent.md`

**Read [`agent.md`](agent.md) now, before writing anything.** It is this repository's real
entry point for any coding agent, and it is tool-agnostic by design. This file exists only
because Claude Code looks for `CLAUDE.md` specifically — `agent.md` asks for exactly this:

> If your tooling looks for a particular filename and you can only symlink or copy one file,
> make it this one.

`agent.md` is authoritative. Where this file and `agent.md` disagree, `agent.md` wins.
Do not grow this file: it is a pointer, not a second copy of the rules. Adding substance
here creates the drift `agent.md`'s closing section warns against.

## Before you write code

`agent.md` lists seven documents to read in order. The two that most often get skipped, and
that most often cause rework when they are:

- [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md) — binding, not advisory.
- The latest dated folder under [`session/`](session/) — sorted by date descending, open its
  `index.html`. Read the most recent next-session mandate before assuming you know the
  current state.

## The three that get sacrificed under time pressure

Restated from [`agent.md`](agent.md#the-rules-that-never-bend), which states all ten. These
are the ones an agent is most likely to break in its first five minutes, before it has read
anything:

1. **Safety before security before correctness before performance.** In that order, always.
2. **Test-driven, no exceptions.** A failing test exists before the code that makes it pass.
   If you are about to write implementation code with no corresponding test, stop.
3. **No Feature or Story bypasses the assurance spine.** Contracts in
   [`goals/assurance/`](goals/assurance/) come before code, and
   `cargo run -p xtask -- check-assurance-spine` must pass.

## Orienting fast

Run these from `os/` — they are the machine-readable view of the project's state:

```
cargo run -p xtask -- help                     # every subcommand
cargo run -p xtask -- list-fixtures            # every QEMU fixture and its owning TEST-*
cargo run -p xtask -- check-assurance-spine    # contracts, loose ends, status headers
cargo run -p xtask -- check-boot-images        # every AArch64 image variant + clippy (LE-72)
```

`check-boot-images` is not optional before pushing a change to `kernel`, `hal-arm64` or
`pi5-image`: nothing else you run locally — not `cargo test`, not `fmt`, not host clippy —
compiles those crates for the board, and three pushes have already gone out green locally
and red on the runner because of it.

Open defects live in [`goals/assurance/loose-ends.tsv`](goals/assurance/loose-ends.tsv)
(`LE-*`), not in prose. Story and Feature status lives in the `Status:` header of each
document under [`goals/`](goals/) and is machine-checked; see
[`goals/assurance/README.md`](goals/assurance/README.md) for the grammar.
