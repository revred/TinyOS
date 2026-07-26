# TinyOS — 26th July Seed MVP

Status: **founding document — the original concept this project grew from**

## What this document is

This is the seed record of TinyOS: the original ambition, captured as a short founding statement rather than a living spec. Every other document in this repository ([`README.md`](README.md), [`CODING_STANDARDS.md`](CODING_STANDARDS.md), and everything under [`docs/`](docs/)) is the elaboration of what's written here. When those documents evolve, this one doesn't — it's the fixed reference point for *why* the project exists and what it was originally meant to be, so that intent never drifts unnoticed as the design grows. See [`HANDOVER.md`](HANDOVER.md) for how far the project has come since this seed and what to pick up next.

## The original ambition

Build a real-time operating system that:

1. Can communicate with Windows or Linux running on the **same machine**, or run as an **edge device OS** — configurable over CAN bus, USB, or Ethernet.
2. **Looks and behaves like MS-DOS 4+** — a fast, legible, keyboard-driven command environment.
3. Has a **solid multitasking core** that keeps UX/UI strictly separate from real-time control, with strict rules governing how real-time actions are triggered.
4. **Loads onto any laptop of today**, down to **Jetson Nano-class edge devices**.
5. Can **host something like Ollama** to interface with local LLMs.
6. Can **take orders from an LLM** — under strict, auditable control, never as an unsupervised root user.

That is the whole of the seed. No hardware had been purchased, no code had been written, and no repository existed yet — only this statement of intent.

## What the seed deliberately left open

The seed set direction, not detail. Everything below was decided later, as elaboration, and is documented in full elsewhere:

- Which language(s) the OS is written in.
- The exact hardware architecture policy (64-bit only, which targets).
- How the LLM's commands are actually gated (the Agent Command Interface).
- How a co-resident host OS or a remote/wireless controller actually talks to TinyOS (HBP, WCI).
- How the OS is deployed, hot-updated, or rebooted remotely.
- The shell's actual command surface (DOS syntax, POSIX syntax, or both).
- How local inference shares GPU/VRAM/unified memory with real-time work, or splits across multiple devices.
- Coding standards, testing discipline, and the safety/security/correctness/performance priority ordering.

## Why this document exists separately from the README

The README is meant to be read top to bottom as the current state of the design — it changes as decisions are made. This seed document is meant to never change: a future contributor (human or AI) should be able to read six words of this file and know exactly what problem TinyOS was created to solve, even after the README has grown far beyond what's written here. If the project's direction ever needs to be sanity-checked against its original intent, this is the file to read.
