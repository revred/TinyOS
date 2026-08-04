# 06A — The Birth Certificate, and the Gate That Finally Compiles for the Board

Session handover, written 2026-08-04 after the session that executed
[`05A`](05A-the-board-speaks-in-spoors-and-what-a-late-listener-cannot-know.md) §5.
Two of `05A`'s three answers are built; the third is still deliberately not taken. `LE-72`
is closed by the gate it asked for, and **three board runs the same session** (§5) settled
almost all of it on silicon.

---

## 0. The one-paragraph state

`05A`'s mandate was answers **1 and 2** — re-announce the boot epoch, and put a boot epoch in
the frame header — with answer 3 (two-way query/response) explicitly deferred behind the
charter work it needs. Both are delivered as `STORY-P1-10-04`, host-Green, Red-verified:
**14 tests fail against an unwritten implementation**. Every frame now carries a 32-bit epoch
in the four bytes the format reserved for exactly this, the boot prologue is retained outside
the ring and re-announced every five park passes as a verbatim frame marked `RETAINED`, and
Ti64Dink reads all of it — reporting a reboot as a reboot rather than as loss. `LE-72` closed:
`cargo run -p xtask -- check-boot-images` builds **every** AArch64 variant and lints all three
board crates, CI now runs that same subcommand, and the spine banner points at it. **Then the
board settled it**: `BOARD VERDICT 11`–`13` read a boot's state out of a capture that opened
at record 74, watched the epoch change across power cycles, carried two boots through one
window with **0 records lost**, and found the certificate **byte-identical** to a live frame 0.
Spine: 30 Features / 94 Stories / 78 Tests, 75 loose ends (42 open). The card is back **in the
laptop**, carrying `fd966f7514e4`; the board is unpowered.

## 1. What was built

| Piece | Where | What it settles |
|---|---|---|
| Boot epoch in the header | `kernel::spoor_wire` | Every frame self-identifies. A listener can tell boot #7 from boot #8, and a reboot from loss. |
| `FLAG_RETAINED` | `kernel::spoor_wire` | A re-announcement says what it is, and `FrameHeader::expected_next` returns **nothing** for one — the phantom gap is unreachable, not merely discouraged. |
| The boot certificate | `kernel::spoor_stream` | The prologue lives outside the ring, write-once, bounded at 16, re-emitted verbatim every `ANNOUNCE_EVERY = 5` calls. |
| `seed_epoch` / `announce` seam | `hal-arm64::spoor`, `pi5-image` | Two more `#[no_mangle]` symbols, all four now named in the `#[used]` link seam. |
| Park-loop announcement | `hal-arm64::ethernet` | Asked every pass, answered on the kernel's period. Same buffer, same staging region, no new grant. |
| Epoch/retained/boot decoding | `work/tools/ti64dink` | Reports reboots, excludes retained frames from the arithmetic, and **checks the verbatim claim** when a capture holds both halves. |
| `check-boot-images` | `xtask` | `LE-72`. Builds featureless **and** every fixture, lints `hal-arm64` + `kernel --lib` + `pi5-image` for the target. |

## 2. Three decisions worth defending

**The certificate is a consecutive run from `seq = 0`, not "the boot rungs wherever they
are."** A frame header carries one sequence and implies its records follow consecutively, so
a certificate assembled from scattered records would make its own header lie. The buffer
therefore takes once-per-boot rungs while the run from zero is unbroken and closes
**permanently** at the first record that is not one. `BeaconTransmitted` is excluded despite
reading like a boot event — it stamps every pass, so it is stream, not birth.

**The re-announcement is verbatim, with the original sequence numbers.** A re-stamp would
carry fresh sequences and a fresh cost field and would be a *different event* wearing the same
name. This is why the `RETAINED` flag has to be a wire field: a host cannot infer it, because
a legitimately repeated sequence and a stream that restarted look identical.

**The epoch is a change detector and never a boot count** (`LE-74`). It is `CNTVCT_EL0` at
kernel entry, folded, so what varies between boots is how many ticks *firmware* spent before
reaching the kernel. Real variation, but borrowed. A host can say "different boot" with high
probability and can never say "boot number N" or "I missed exactly two". The alternative was
dressing a low-entropy sample up as a nonce, which would have made the field read stronger
than it is — the exact failure mode `ADR 0005` exists to prevent.

## 3. `LE-72`, and why the gate is shaped this way

The row was rewritten in `05A`'s session to say that **"the AArch64 build" is plural**, after a
third red push arrived past the gate adopted to stop the first two. `xtask pi5 --fixture=measure`
was structurally incapable of catching that instance: CI builds the image *featureless*, and
that subcommand cannot produce a featureless image at all.

So the gate builds the list, and the list is a **pure function held against the fixture
register by a host test** — a fixture registered and not built fails in milliseconds instead of
on a runner in minutes. Clippy widened from `-p hal-arm64` to all three board crates, with
`kernel` scoped to `--lib` because its `[[bin]]` is the x86_64 Tier 0 guest; a test pins that
as the only legitimate narrowing, so it cannot become a way to hide a failure.

**CI now runs the same subcommand** instead of its own two steps. That was the actual defect:
the local gate and the runner drifting apart. And it is habitual three ways — the
`check-assurance-spine` banner ends by naming it, and both `agent.md` and `CLAUDE.md` carry it.

## 4. Also found

- **`LE-73`** — `kernel::udp_wire` is documented as `STORY-P1-10-03` and **no such Story
  exists**: no document, no contract row, no Test. A fully implemented, 8-test module is
  joined to the spine by a citation that resolves to nothing, and every existing gate is blind
  to it because they check documents against each other and nothing extracts `STORY-*`
  citations from source. Same prose-versus-register class as `LE-65` and `LE-70`, one layer
  further out. **This session's Story is `-04` to avoid colliding with the phantom.**
- **`LE-74`** — the epoch's entropy limit, above.
- Host clippy on this Windows machine still cannot build `kernel`'s `[[bin]]` (`hal_x86_64`
  modules are `cfg(not(windows))`). Pre-existing, `LE-64`'s class, unchanged by this work —
  but it means `cargo clippy --workspace --all-targets` is **not** a clean local signal here,
  and `check-boot-images` is the one that is.

## 5. The board evidence — taken the same session

All three runs happened before this handover was filed. Full transcripts and per-field
reasoning are in
[`pios-ground-truth-2026-08-03.txt`](../../goals/reports/pios-ground-truth-2026-08-03.txt).

| Verdict | The capture shape | What it settled |
|---|---|---|
| 11 | Power on, wait ~30 s with **nothing listening**, then capture 30 s | Opened at record **74**, frame 0 long gone — and read `MmuEnabled cost=184052`, `GicRouted`, `TickArmed` out of a retained certificate anyway. 6 retained frames in 30 s (one per ~5 s, matching `ANNOUNCE_EVERY`). **0 lost.** |
| 12 | Fresh boot, capture started at power-on | Epoch changed `0x049F8B28` → `0x04B328BC`. First time this board could say which boot a frame belongs to. Still missed frame 0 — the stream had reached `seq=16` before pcap opened. |
| 13 | One capture **spanning** a power cycle | Two boots, **0 records lost** across a restart that took the sequence from 244 back to 0. And the one that matters: `boot state : captured live AND re-announced — 3 record(s) byte-identical`. |

**Verdict 13 is the verbatim claim checked rather than asserted.** That window holds one
boot's live frame 0 *and* its certificate, and Ti64Dink compared them record for record.
Criteria 1–5 and 7 are Green on silicon; criterion 6 (bounded, write-once) stays host-Green
because no board run has stamped enough certificate rungs to reach the ceiling.

**And the runs sharpened `LE-74` rather than merely citing it.** Three consecutive boots gave
`0x049F8B28`, `0x04B328BC`, `0x04B32825` — the last two **151 counter ticks apart, 2.8 µs at
54 MHz.** Only the low byte moved. The epoch is a sound bench-session change detector and is
measurably not an identifier; the row now carries the measurement instead of the caution.

Two honest notes. Verdict 13's `== BOOT CHANGED ==` line is absent from the transcript because
the run was piped through `tail -90` and it scrolled off — a defect in the evidence plumbing,
not in the subject, and the summary proves the transition was handled. And **the link is still
egress-only**: nothing here is a bidirectional exchange, `gem.rs` still enforces
`no_path_in_this_module_ever_enables_receive`, and `LE-67`'s containment story is untouched.
The whole point of this Story is that a listener never *has* to ask.

## 5.1 What the board did not do: the fan never spun

Owner observation during the session, and it is filed as **`LE-75`** rather than explained
away. The Pi 5's fan header is driven by RP1 PWM, which Linux binds to a thermal zone;
TinyOS has never programmed RP1 PWM and **has no thermal sensing of any kind** — it cannot
read the SoC temperature, cannot throttle, and does not know whether the firmware is managing
heat on its behalf.

That touches rule 1 — safety before everything. These captures are 30–90 seconds each and
none of them is evidence about sustained operation. `05A` §7 recorded the board left
"powered, beaconing" between sessions; **that is the practice this row argues against** until
the sensor is read. The remediation is sensing before actuation: put the temperature on the
spoor stream as its own rung first, then drive the fan *from that reading* rather than from a
hardcoded duty cycle — the discipline `LE-69` was closed under.

## 6. Still owed, unchanged from `05A` §6

- **`REPORT-2026-08-04-01`**, closing `LE-09` (release-blocking), `LE-15`, `LE-24`, `LE-27`.
  Not from the photographs — Ti64Dink and `xtask parse-meas` quote machine-parsed bytes.
- **`FEAT-P1-09`'s exit criterion** — the beacon byte-compared against the frame builder.
- **`STORY-P1-10-02` criterion 6** — per-stamp and per-drain cost still unmeasured. The
  per-announce cost joins it.
- **`LE-56`'s shell-lane half** — the board half is evidenced; the console lane is untouched.
- **`udp_wire` is still not wired to the board**, and now also has no Story (`LE-73`).

## 7. Bench facts at close

- **Card: in the laptop, TOS64 role, `fd966f7514e4`**; board unpowered at close, hash-verified on the card by
  `tos64-cardswap`. `pios-backup\` untouched. The previous image was `b44040659702`.
- `MAX_RECORDS` is **181**, and the stale `184` was corrected in the architecture document,
  `STORY-P1-10-01` and Ti64Dink — the C# decoder had been bounding `count` against 184, which
  was three records looser than the format it was guarding.
- The frame header's four reserved padding bytes are **spent**. There is no room left for a
  future field that does not move the records; the `flags` word has 15 bits.
- `ANNOUNCE_EVERY = 5` and `CERTIFICATE_CAPACITY = 16` are **chosen, not measured**, and both
  are recorded as debt rather than presented as findings.
- `LINK=DOWN BEACON=SKIPPED` on the report row is still the expected boot snapshot. Do not
  re-diagnose it.
- **The fan does not spin under TinyOS** (`LE-75`). Power the board for a run and power it
  down after; nothing measured supports leaving it running.
- **Three board epochs on record**: `0x049F8B28`, `0x04B328BC`, `0x04B32825`. The last two are
  151 counter ticks apart.
