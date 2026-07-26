# STORY-P0-04-03 — Minimal Bus-Enumeration Pass

Status: **Planned, not yet started**
Feature: [`FEAT-P0-04`](../features/FEAT-P0-04.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

A minimal PCI(e) bus-enumeration pass — walking configuration space to discover attached devices and record them in the topology model `STORY-P0-04-01` established — sufficient groundwork for the class drivers `EPIC-P3` plans, without implementing any actual device driver here.

## Depends on

`STORY-P0-04-01` (the topology model devices are recorded into).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. Enumeration under QEMU's `q35` model discovers at minimum the host bridge and any devices QEMU's default machine exposes, recorded with vendor/device ID and topology position (bus/device/function).
2. Enumeration is read-only against device configuration space at this stage — no driver bring-up, no device state mutation — keeping this Story's scope to discovery only, per Single Responsibility.

## Tests

Not yet written — deferred until this Story is picked up. Requires a Tier 0 (QEMU) integration test.

## Goals verified

G-HW-4; groundwork for `EPIC-P3`'s class drivers (not itself a G-HW-2/G-PA-4 Goal owner).
