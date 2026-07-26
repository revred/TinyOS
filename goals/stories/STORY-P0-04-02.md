# STORY-P0-04-02 — Interrupt Controller (APIC) Bring-Up

Status: **Planned, not yet started**
Feature: [`FEAT-P0-04`](../features/FEAT-P0-04.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

Bring up the local APIC and I/O APIC (routing information sourced from `STORY-P0-04-01`'s MADT parse), replacing the legacy 8259 PIC path with the interrupt-routing model the scheduler's timer tick (`FEAT-P0-02`) and future drivers (`EPIC-P3`) both need.

## Depends on

`STORY-P0-04-01` (MADT-derived routing information).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A timer interrupt configured through the local APIC fires at a bounded, measured interval under QEMU — verified by a Tier 0 test, not assumed from datasheet timing alone.
2. Spurious/unrouted interrupts are handled explicitly (a documented default handler), never silently ignored in a way that could mask a real hardware fault.

## Tests

Not yet written — deferred until this Story is picked up. Requires a Tier 0 (QEMU) integration test.

## Goals verified

G-HW-4; indirectly supports G-RT-1 (the scheduler's tick source).
