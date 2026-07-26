# Extended Domain Workloads — From Washing Machines to Rotary Detonation Engines

Status: **draft / vision-tier document — deliberately tiered by realism, not a uniform roadmap commitment**

## Purpose

[`docs/physical-ai-reference-workloads.md`](physical-ai-reference-workloads.md) proved that one RT core serves genuinely different control problems (a 5-axis CNC, a Wire DED robot arm, a resin-curing UV array) through a small set of shared primitives rather than bespoke subsystems per workload. This document extends that same question — "does the architecture actually generalize, or does it just happen to fit three examples that were chosen to fit it?" — against a much wider and much harder set of domains: a washing machine, a grid-interactive solar battery charger, a high-torque axial-flux motor, a sea-landing rocket booster controller, internal-combustion valve-timing engine management, a Wankel rotary engine, an automatic transmission, a drone flight controller, a liquid-piston (inverse Wankel) engine, and a rotary detonation rocket engine.

**This document is explicitly not a uniform commitment.** Some of these are realistic near-term extensions of the existing architecture. Some are credible but would need real domain investment. And some — most notably the rocket landing controller and the rotary detonation engine — are extreme-tail demonstrations of where the architecture's ceiling might be, included because the question "could it, in principle, extend this far" is worth answering honestly, not because TinyOS is committing to build a rocket engine controller. Overselling that would be exactly the kind of mistake this project's other documents have deliberately avoided (see, for comparison, how [`docs/universal-driver-model.md`](universal-driver-model.md) handles the Apple Silicon hardware gap: stated plainly, not glossed over).

## Four additional shared RT primitives

The three-workload document specified four primitives: Motion & Interpolation, Process-Synchronized Output, Position Feedback Abstraction, and Safety Interlock. Surveying the ten new domains below surfaces four more that the architecture needs to claim genuine generality — each is a new *kind* of primitive, not a variant of an existing one:

### 5. State-Machine / Sequencer Service

- Many real controllers are not continuous-trajectory problems at all — they're **discrete-event, mode-based sequences**: a washing machine cycle (fill → wash → drain → rinse → spin) or an automatic transmission's gear-shift logic (a decision tree over throttle position, road speed, and load, executed as a bounded-time state transition). This is a different computational shape from motion interpolation, and pretending it's "just motion with one axis" would be a category error.
- The service manages a declared state graph with guarded transitions (a transition fires only when its guard condition — sensor readings, elapsed time, operator command — holds), each transition and each state's dwell behavior carrying its own WCET budget like any other RT-scheduled work.
- This primitive is what makes the washing machine and automatic transmission genuinely simple to specify on TinyOS — they need no new kinematics or interpolation machinery at all, just a sequencer and a modest set of I/O class drivers (valve solenoids, drum motor, water-level/temperature sensors for the washing machine; shift solenoids, speed/torque sensors for the transmission).

### 6. High-Frequency Closed-Loop Stabilization Service

- Distinct from Motion & Interpolation's "follow a programmed path": this primitive continuously estimates a system's state from multiple sensors (gyroscope, accelerometer, magnetometer, barometer/GPS for a drone; inertial and possibly optical/radar sensing for a rocket booster) and computes a correction at a very high, fixed update rate to hold or steer toward a target attitude/rate/trajectory that is itself often being adjusted in real time — the target moves, unlike a CNC's pre-programmed path.
- This is the primitive a **drone flight controller** exercises centrally: attitude/rate control loops commonly run at update rates far higher than the CNC's interpolation tick rate, with sensor fusion (combining multiple noisy, partially-redundant sensors into one state estimate) as a first-class concern the CNC/Wire DED/resin workloads never needed.
- The **rocket landing controller** exercises the same primitive at a much harder tier: guidance (deciding where the vehicle should be) is coupled to control (getting it there) under extreme time pressure, with a moving landing target (a sea-based pod, itself subject to wave motion) and a vehicle whose mass and center of gravity change continuously as propellant burns — see the Tier C discussion below for why this is treated as an extreme-tail case, not a near-term commitment.

### 7. Combustion / Ignition Event-Timing Service

- Internal combustion engine management (valve timing, ignition timing) requires firing events referenced not to wall-clock time but to **crank angle** (or, for a Wankel/liquid-piston rotary design, to the equivalent rotor-angle reference) — a fundamentally different timing reference from anything else in this document, and one that gets *more* demanding, not less, as engine speed increases, because the same angular window shrinks in wall-clock time.
- The service exposes a rotating-reference-based scheduling primitive: "fire this output N degrees after top-dead-center, at whatever the current rotational speed implies for wall-clock timing," recomputed continuously from a real-time angular position/speed measurement (a variant of the Position Feedback Abstraction, but angle- rather than linear-position-based).
- The **rotary detonation engine** case pushes this primitive to its most extreme form: rather than a single, relatively low-frequency combustion event per cycle (a few hundred to a few thousand times per minute, as in a conventional or Wankel engine), a rotating detonation wave repeats at a very high frequency, and injector/ignition sequencing must stay synchronized to that wave with correspondingly tighter timing margins — this is the single hardest timing requirement anywhere in this document, harder than the 5-axis CNC's interpolation tick rate by a wide margin, and is treated accordingly in the tiering below.

### 8. Power Electronics / Grid-Interconnect Control Service

- **Motor drive control** (the axial-flux motor case) and **grid-interactive power conversion** (the solar-charger-that-resells-to-the-grid case) both center on high-frequency switching control loops (commutation/field-oriented control for the motor; MPPT — maximum power point tracking — and inverter switching for the solar/grid case) plus a class of safety interlock neither the Physical AI document's e-stop pattern nor the ignition-timing primitive quite covers: **anti-islanding and grid-synchronization interlocks** — a grid-tied inverter must not energize the grid side when the grid is down (a safety requirement for utility line workers, not just an equipment-protection one) and must phase-lock its output to the grid before connecting.
- This primitive generalizes the Safety Interlock pattern from the Physical AI document to an electrical-domain-specific interlock class, alongside the existing motion-active and exposure-window interlock patterns already specified.

## The ten domains, tiered by realism

| Domain | Primary primitive(s) exercised | Tier |
|---|---|---|
| Washing machine | State-Machine/Sequencer, basic I/O class drivers | **A — near-term credible** |
| Automatic transmission gear control | State-Machine/Sequencer, Position/speed feedback | **A — near-term credible** |
| ICE valve timing / engine management | Combustion/Ignition Event-Timing, State-Machine/Sequencer (idle/load modes) | **A — near-term credible** |
| Drone flight controller | High-Frequency Closed-Loop Stabilization, Safety Interlock (geofence/failsafe) | **A — near-term credible, highest engineering effort within this tier** |
| Axial-flux motor (high-torque) drive | Power Electronics/Grid-Interconnect Control (motor-drive variant), Combustion/Ignition-style angle-referenced commutation timing | **B — credible with real domain investment** |
| Solar-grid battery charger + grid resale | Power Electronics/Grid-Interconnect Control (grid-tied variant), Safety Interlock (anti-islanding) | **B — credible with real domain investment** |
| Wankel rotary engine | Combustion/Ignition Event-Timing (rotor-angle reference instead of crank-angle) | **B — credible with real domain investment** |
| Liquid-piston (inverse Wankel) engine | Combustion/Ignition Event-Timing, novel kinematics reference (least standardized of the set) | **B — credible with real domain investment, less prior art to lean on than the Wankel case** |
| Rocket landing controller (sea-based pod) | High-Frequency Closed-Loop Stabilization at its hardest tier, LLM-assisted supervisory replanning (see below) | **C — extreme-tail aspirational** |
| Rotary detonation rocket engine | Combustion/Ignition Event-Timing at its hardest tier (highest-frequency case in the whole document) | **C — extreme-tail aspirational** |

### What distinguishes Tier A from Tier B

Tier A domains need **no new architectural primitive beyond what's specified above**, and their I/O (solenoids, simple motor drives, standard automotive-class sensors, standard drone IMU/GPS/ESC interfaces) falls within the [Universal Driver Model](universal-driver-model.md)'s existing class-driver categories or clear near-term extensions of them. A capable team could plausibly reach a real, hardware-validated demonstration of any Tier A domain on a timeline comparable to the 5-axis CNC flagship, using the same MVP hardware pair as a starting point plus domain-appropriate I/O peripherals.

Tier B domains are architecturally covered by the primitives above, but each requires real domain expertise TinyOS's current design work doesn't yet encode: motor-drive field-oriented control tuning, grid-interconnection regulatory/safety standards compliance, and — for the two rotary-engine cases — combustion dynamics expertise that has essentially no representation in this project today. These are credible, not currently resourced.

### Why Tier C is treated differently, explicitly

The rocket landing controller and the rotary detonation engine are included in this document on purpose, not to pretend TinyOS is close to either, but because refusing to even discuss how the architecture *would* need to stretch to reach them would hide exactly the kind of ceiling this document exists to surface. Three things make Tier C categorically different from Tier B, not just "harder":

1. **Consequence of failure is categorically higher**, and the engineering discipline required (aerospace-grade guidance, navigation, and control certification processes; propulsion combustion-instability analysis) is a specialized field in its own right that this project has made no claim to competence in. Building a real controller for either domain would require partnering with domain experts, not extending TinyOS's existing docs by analogy.
2. **The timing/physics margins are at or beyond what the architecture's existing primitives have been designed and reasoned about for.** The Combustion/Ignition Event-Timing service's design above was informed by conventional and Wankel-class engine timing; a rotating detonation wave's frequency is high enough that whether the same primitive design holds, or needs fundamentally different scheduling guarantees, is an open research question this document does not resolve — stated honestly rather than assumed away.
3. **The LLM-assisted pattern applies differently here than anywhere else in this document.** For every other domain, the ACI's "LLM proposes, TinyOS decides" pattern (README Design Pillar 5) means an agent might suggest a wash cycle adjustment or a flight-path waypoint change, gated by the same policy engine as any caller. For a rocket landing, the honest framing is narrower: an LLM (local or external, per [`docs/inference-architecture.md`](inference-architecture.md)) could plausibly assist at the **guidance replanning layer** — proposing an updated trajectory or landing-burn profile in response to changing conditions (wind, pod drift, an engine-out condition) — strictly as a proposal the deterministic control layer evaluates against hard safety bounds before acting on, never as something with any path to directly actuating a thrust vector or throttle command. This is the existing Non-Negotiable #2 pattern, stated in its most consequential possible application, specifically to make clear that even at this extreme tier, the rule does not bend.

## What this document commits to, and what it doesn't

- **Commits to**: the four new primitives above being a coherent, honest extension of the architecture, and the Tier A domains being genuinely near-term-credible on the existing MVP hardware plus domain-appropriate I/O.
- **Does not commit to**: a roadmap phase, a timeline, or a hardware purchase for any Tier B or Tier C domain. None of the ten domains in this document are in the [MVP scope](../SeedMVP.md#5-narrowing-to-the-mvp-configuration) defined by the master specification — that remains the 5-axis CNC (flagship), Wire DED arm, and resin-curing array from `docs/physical-ai-reference-workloads.md`.
- **Does not commit to**: TinyOS being a credible platform for actual rocket flight or actual detonation-engine control without substantial, dedicated domain partnership and safety-certification work far beyond what a software architecture document can establish on its own.

## Status

This document is a vision-tier extension of the goal taxonomy in [`SeedMVP.md`](../SeedMVP.md) (Section 3.1, Physical AI Goals) and the primitive architecture in `docs/physical-ai-reference-workloads.md`. It should be read as evidence toward "how general is this architecture, really," not as an additional set of committed reference workloads.
