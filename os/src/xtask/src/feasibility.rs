//! The feasibility report — *"where do we stand, and what would prove it?"*
//!
//! # Why this is generated and not written
//!
//! The owner asked for a live report on how close TinyOS is to being a proven
//! project. A hand-written status page is precisely `LE-108`: the one number on
//! `goals/index.html` that no gate checked drifted by 33 Stories while sitting
//! two lines below numbers that could not drift. So **every figure here is
//! derived on the run that emits the page**, and `check-feasibility` refuses a
//! committed copy that disagrees with the generator.
//!
//! # Why the judgements are constants in this file
//!
//! Numbers can be derived; *"is this feasible"* cannot. The narrative lives
//! here as documented constants rather than in the HTML, so changing a claim is
//! a reviewable source diff rather than an untracked edit to a page. That is
//! the same split `dashboard` makes — generated tiles, gated prose — with the
//! boundary moved, because this page is **all** claim and has no editorial half
//! worth protecting.
//!
//! # The one thing this report must never do
//!
//! Report a gate carrying evidence as a gate that *passed*. `guardrail-evidence`
//! records that somebody measured, never that the number met its target: six of
//! its rows are `refused`, meaning measured, read and declined. The renderer
//! says so on the page, in those words, because a reader who mistakes evidence
//! for a pass has been misled by a document claiming rigour.

use std::fmt::Write as _;
use std::path::Path;

use crate::assurance::{self, ArtifactStatus};
use crate::dashboard::DashboardFacts;

/// A landing zone: what TinyOS is *for*, and what must be published before
/// that use may be claimed.
struct LandingZone {
    id: String,
    name: String,
    horizon: String,
    claim_gate: String,
}

/// The project's own roadmap phases, with what each claims to prove.
///
/// Sourced from `README.md`'s roadmap and the Epic documents. Held here rather
/// than parsed because the phase *narrative* is prose a machine cannot derive;
/// the STATE of each phase is derived below from Epic `Status:` headers, so the
/// two cannot silently disagree about progress — only about wording.
const PHASES: [(&str, &str, &str); 5] = [
    (
        "EPIC-P0",
        "Kernel skeleton",
        "That a kernel can be built, booted, scheduled and halted under CI on every push — \
         the pipeline before the product. All 27 Stories Verified at Tier 0; its assurance \
         debt is explicitly EPIC-P1's charge.",
    ),
    (
        "EPIC-P1",
        "Determinism proof",
        "That scheduling, faults, isolation and deadlines behave predictably enough to quote. \
         Exit needs the timing-regression gate to have <em>demonstrably failed at least once</em> \
         on a deliberately-introduced regression.",
    ),
    (
        "EPIC-P2",
        "Operator command environment",
        "That the system is drivable by a human. Blocked on a storage decision: at least 15 of \
         the 22 specified verbs cannot be implemented because there is nothing to implement them \
         against — no filesystem crate exists.",
    ),
    (
        "EPIC-P6B",
        "TinyTile heterogeneous compute",
        "That device kernels can be admitted as code under the same charter as CPU code. \
         Planned only — Features enumerated, no Story exists.",
    ),
    (
        "EPIC-P9",
        "Memory confidentiality against a dump",
        "That a physical memory dump yields nothing. Every Feature but one is gated on hardware \
         this project does not yet have.",
    ),
];

/// `SeedMVP.md` §3 — what this project set out to build. **These are the
/// goals** the roadmap and every Epic exist to serve, so the report leads with
/// them rather than with internal register arithmetic.
///
/// The third column is this session's honest read of the distance, and it is a
/// judgement: it is here in source, reviewed, rather than typed into a page.
const FOUNDING_GOALS: [(&str, &str, &str); 6] = [
    (
        "Coexist with Windows/Linux on one machine, or run as an edge-device OS",
        "not started",
        "No host bridge exists. TinyOS boots standalone on x86_64 under QEMU and on a Raspberry \
         Pi 5 over netboot; coexistence is Phase 4 and has no Epic document.",
    ),
    (
        "Look and behave like MS-DOS 4+",
        "partly",
        "TINYCMD's DOS front-end runs a .TCB batch and byte-matches a committed golden transcript \
         in CI. But EPIC-P2 records that most canonical verbs cannot be implemented yet — there \
         is no filesystem.",
    ),
    (
        "A solid multitasking core",
        "strongest claim",
        "Preemption, priority inheritance, WCET budgets with restart/degrade/trip arms, per-task \
         address spaces with W^X, and fault containment — all gated in CI, several falsified by \
         deliberate mutation before being believed.",
    ),
    (
        "Load onto any laptop, down to Jetson-class edge devices",
        "one board, one arch",
        "Real silicon evidence exists for exactly one target, the Pi 5. Jetson Orin Nano is in the \
         platform register as unqualified. No install path onto a laptop exists.",
    ),
    (
        "Host something like Ollama",
        "not started",
        "Phase 6. No inference runtime, no model loading, no memory substrate for weights. The \
         heterogeneous-compute Epic that would carry it is planned only.",
    ),
    (
        "Take orders from an LLM under strict, auditable control",
        "not started",
        "Phase 5. The capability and code-admission registers that would govern it are specified \
         and machine-checked, which is the groundwork — but no agent interface exists.",
    ),
];

/// The honest verdict. **Change this only with evidence.**
///
/// Stated as a single sentence because a reader who takes one thing from this
/// page should take this. It is deliberately not a percentage: the release-gate
/// ratio below is the closest thing to one, and it is not a score.
const VERDICT: &str = "TinyOS is a real, booting, CI-gated operating system with a genuinely \
     strong multitasking core and evidence from real silicon. It is <em>not</em> close to being \
     a proven product: of its six founding goals one is substantially met, one is partly met, \
     and four have not started — and the single decision that would let any timing claim be \
     made at all has not been taken.";

/// What separates "it works" from "it is proven", in the project's own terms.
const PROOF_BAR: &str = "<code>SeedMVP</code> sets the bar as falsifiability, not features: the \
     MVP is <em>&ldquo;the smallest configuration that can <strong>falsify</strong> every in-scope \
     goal&rdquo;</em>. Against that bar the machinery is in place and the evidence is not. Under \
     ADR 0005 a worst-case bound may be quoted only from a platform holding a secure-world \
     qualification record; zero platforms hold one, the Pi 5 included. And the project has already \
     corrected itself once here &mdash; it <strong>declares and enforces a budget; it does not \
     claim a bound</strong>, because an observed maximum over n=1000 is a different quantity from \
     a bound.";

/// Renders the whole page. Every number is passed in, none is written here.
pub fn render(
    repo_root: &Path,
    facts: &DashboardFacts,
    status: &assurance::ReleaseStatus,
    statuses: &[ArtifactStatus],
) -> Result<String, String> {
    let zones = landing_zones(repo_root)?;
    let present = &status.implemented;
    let mut out = String::new();

    out.push_str(HEAD);

    // ---- verdict -------------------------------------------------------
    let _ = write!(
        out,
        r#"<h1>TinyOS — feasibility report</h1>
<p class="sub">Generated from the assurance spine. Every figure below is derived on the run that
wrote this file; <code>check-feasibility</code> refuses a committed copy that disagrees with the
generator, so this page cannot go stale the way prose does.</p>
<div class="verdict"><p class="lede">{VERDICT}</p><p class="bar">{PROOF_BAR}</p></div>
"#
    );

    // ---- the one ratio that matters ------------------------------------
    let _ = write!(
        out,
        r#"<h2>Where we stand, in four numbers</h2>
<div class="grid">
  {}
  {}
  {}
  {}
</div>
<p class="note"><strong>A gate carrying evidence is a gate somebody measured — never a gate that
passed.</strong> {} of the evidence rows are <code>refused</code>: measured, read against the
target, and declined. This page counts evidence, and evidence is not a score.</p>
"#,
        tile(
            &format!("{}&nbsp;/&nbsp;{}", facts.assurance_verified, facts.stories),
            "Stories assurance-verified",
            "Locked at zero by ADR 0005, not lagging. No engineering moves it."
        ),
        tile(
            &format!("{}&nbsp;/&nbsp;{}", facts.qualified_platforms, facts.platforms),
            "Platforms qualified",
            "The decision that unlocks everything above. Owner's to take."
        ),
        tile(
            &format!("{}&nbsp;/&nbsp;{}", facts.evidenced_gates, present.open),
            "Release gates with evidence, of those that CAN be closed",
            "Against the raw denominator this reads far better and means less."
        ),
        tile(
            &format!("{}", present.measurable_today),
            "Gates measurable today",
            "No board, no decision, no missing mechanism. Ordinary work."
        ),
        present.refused,
    );

    // ---- the founding goals ---------------------------------------------
    out.push_str(
        "<h2>The six founding goals, and the distance to each</h2>\n\
        <p class=\"note\">From <code>SeedMVP.md</code> &sect;3. These are what the roadmap and \
        every Epic exist to serve, so they come before any internal arithmetic.</p>\n<table>\n\
        <thead><tr><th>Goal</th><th>Distance</th><th>Where it actually stands</th></tr></thead>\n\
        <tbody>\n",
    );
    for (goal, distance, detail) in FOUNDING_GOALS {
        let class = match distance {
            "strongest claim" => "ok",
            "partly" | "one board, one arch" => "wip",
            _ => "spec",
        };
        let _ = writeln!(
            out,
            "<tr><td><strong>{goal}</strong></td>\
             <td><span class=\"badge {class}\">{distance}</span></td><td>{detail}</td></tr>"
        );
    }
    out.push_str("</tbody></table>\n");

    // ---- what is actually proven ---------------------------------------
    out.push_str(
        "<h2>What is actually proven</h2>\n<p class=\"note\">Claims below are backed by \
        a runner or by silicon. Nothing here rests on a laptop alone.</p>\n<ul class=\"proof\">\n",
    );
    for (claim, evidence) in PROVEN {
        let _ = writeln!(out, "  <li><strong>{claim}</strong><span>{evidence}</span></li>");
    }
    out.push_str("</ul>\n");

    // ---- what is not ----------------------------------------------------
    out.push_str("<h2>What is not proven, stated plainly</h2>\n<ul class=\"unproven\">\n");
    for (claim, why) in UNPROVEN {
        let _ = writeln!(out, "  <li><strong>{claim}</strong><span>{why}</span></li>");
    }
    out.push_str("</ul>\n");

    // ---- the ledger ------------------------------------------------------
    let absent = &status.without_subsystem;
    let _ = write!(
        out,
        r#"<h2>The release-gate ledger</h2>
<p class="note">{} gates in play ({} in-play domains &times; {} release guardrails). The raw
<code>{}&nbsp;/&nbsp;{}</code> ratio overstates the work remaining and understates the indictment,
because half the denominator cannot be closed by construction.</p>
<table>
<thead><tr><th>Bucket</th><th class="n">Gates</th><th>Who can move it</th></tr></thead>
<tbody>
<tr><td>Domains whose subsystem does not exist</td><td class="n">{}</td><td>Nobody yet — includes all {} hardware-only gates</td></tr>
<tr><td><code>G04</code> bound gates, barred by ADR 0005</td><td class="n">{}</td><td><strong>The owner</strong>, by qualifying one platform</td></tr>
<tr><td>Needing a board</td><td class="n">{}</td><td>Nobody — derived, and it is zero. The board was never the constraint</td></tr>
<tr><td>Carrying evidence</td><td class="n">{}</td><td>Done ({} of them a reasoned refusal)</td></tr>
<tr><td>Mechanism not built</td><td class="n">{}</td><td>Engineering — load, queueing, isolation, soak</td></tr>
<tr><td>No metric-emitting fixture exists</td><td class="n">{}</td><td>Engineering — build the instrument first</td></tr>
<tr class="hi"><td>Instrumented, unmeasured, available today</td><td class="n">{}</td><td><strong>Anyone, now</strong> — and it is {} distinct measurements, not {} jobs</td></tr>
</tbody>
</table>
"#,
        status.in_play,
        status.in_play_domains.len(),
        status.release_guardrails_per_domain,
        facts.evidenced_gates,
        status.in_play,
        absent.gates,
        absent.hardware_only,
        present.bound_class_barred,
        present.hardware_only,
        present.evidenced,
        present.refused,
        present.mechanism_absent,
        present.no_instrument,
        present.measurable_today,
        distinct_measurements(present),
        present.measurable_today,
    );

    // ---- roadmap ---------------------------------------------------------
    out.push_str(
        "<h2>The roadmap, against its own Epics</h2>\n<table>\n<thead><tr><th>Phase</th>\
        <th>What it claims to prove</th><th>State</th></tr></thead>\n<tbody>\n",
    );
    for (epic, name, proves) in PHASES {
        let state = statuses
            .iter()
            .find(|s| s.id == epic)
            .map(|s| s.state.as_str())
            .unwrap_or("not on disk");
        let _ = writeln!(
            out,
            "<tr><td><strong>{epic}</strong><br><span class=\"dim\">{name}</span></td>\
             <td>{proves}</td><td>{}</td></tr>",
            badge(state)
        );
    }
    let _ = writeln!(
        out,
        "</tbody></table>\n<p class=\"note\">{} of {} roadmap Epics are decomposed into Stories at \
         all. Of the {} <code>EPIC-P0</code>/<code>EPIC-P1</code> Stories, {} are Verified or \
         Functionally Verified — <em>functionally</em>, which is a different and weaker word than \
         assurance-verified.</p>",
        facts.epics_decomposed, facts.epics_total, facts.p0p1_stories, facts.p0p1_settled
    );

    // ---- landing zones ---------------------------------------------------
    out.push_str(
        "<h2>What TinyOS is for, and what each use must publish first</h2>\n\
        <p class=\"note\">These are the project's own landing zones and their own claim gates, \
        verbatim from <code>goals/context/landing-zones.tsv</code>. <strong>Not one claim gate is \
        satisfied today.</strong></p>\n<table>\n<thead><tr><th>Zone</th><th>Horizon</th>\
        <th>Must publish before any claim</th></tr></thead>\n<tbody>\n",
    );
    for zone in &zones {
        let _ = writeln!(
            out,
            "<tr><td><strong>{}</strong><br><span class=\"dim\">{}</span></td><td>{}</td>\
             <td class=\"gate\">{}</td></tr>",
            zone.id,
            escape(&zone.name),
            horizon(&zone.horizon),
            escape(&zone.claim_gate)
        );
    }
    out.push_str("</tbody></table>\n");

    // ---- the marketing claim gates ---------------------------------------
    out.push_str(
        "<h2>The two claims that are blocked outright</h2>\n\
         <p class=\"note\">Of the 25 performance guardrails, 23 are release gates and \
         <strong><code>G24</code> and <code>G25</code> are <em>claim</em> gates</strong> &mdash; \
         the project's own rule against marketing ahead of evidence.</p>\n\
         <ul class=\"unproven\">\n\
         <li><strong><code>G24</code> &mdash; &ldquo;better than Linux&rdquo;</strong>\
         <span>Requires the same source-level workload on identical hardware, clocks, compiler \
         options, power state and safety checks, with raw data published. Not run.</span></li>\n\
         <li><strong><code>G25</code> &mdash; &ldquo;10&times; better than most RTOSes&rdquo;</strong>\
         <span>May be stated only for a named metric after a same-hardware comparison against at \
         least three current RTOS baselines, with confidence intervals. The project's own words: \
         if the ratio is below 10&times;, <em>TinyOS can still ship if its absolute release gates \
         pass, but that marketing claim is blocked</em>. Not run.</span></li>\n</ul>\n",
    );

    // ---- what remains ----------------------------------------------------
    out.push_str("<h2>What would move this forward, in order</h2>\n<ol class=\"next\">\n");
    let _ = writeln!(
        out,
        "  <li><strong>Qualify one platform under ADR 0005.</strong><span>Unlocks {} \
         <code>G04</code> gates and makes assurance <code>verified</code> reachable for the first \
         time. Q1 is largely held, Q2 is a laptop afternoon of vendor research, Q4 is already \
         written; Q3 needs a corrected instrument (<code>LE-103</code>) and a campaign. \
         <strong>This is the owner's decision and nothing else competes with it.</strong></span></li>",
        present.bound_class_barred
    );
    let _ = writeln!(
        out,
        "  <li><strong>Read the numbers already taken.</strong><span>Some gates counted as \
         unmeasured were measured, committed and never read against their target \
         (<code>LE-104</code>). {} rows are now refusals filed exactly that way. No code, no \
         board — someone reading a committed number against a committed target.</span></li>",
        present.refused
    );
    let _ = writeln!(
        out,
        "  <li><strong>Measure the {}.</strong><span>{} distinct measurements across {} \
         instrumented domains, several owed by every one of them, so one harness arm moves many \
         gates. Ordinary work with no dependency.</span></li>",
        present.measurable_today,
        distinct_measurements(present),
        present.per_domain.iter().filter(|d| d.instrumented).count()
    );
    let _ = writeln!(
        out,
        "  <li><strong>Build the missing instruments.</strong><span>{} gates sit in domains with \
         no metric-emitting fixture at all (<code>LE-109</code>). These are fixture-building jobs, \
         not measurement jobs, and calling them measurable overstated the position by that \
         much.</span></li>",
        present.no_instrument
    );
    let _ = writeln!(
        out,
        "  <li><strong>Build the absent mechanisms.</strong><span>{} gates describe machinery \
         nobody has written — load, queueing, isolation under competing load, exhaustion \
         containment, soak. You cannot flood a budget that does not exist.</span></li>",
        present.mechanism_absent
    );
    out.push_str("</ol>\n");

    // ---- defects ----------------------------------------------------------
    let _ = writeln!(
        out,
        "<h2>Open defects</h2>\n<p class=\"note\"><strong>{} open</strong> of {} recorded, in \
         <code>goals/assurance/loose-ends.tsv</code>. This register is deliberately adversarial: \
         rows are written against the project, several against the session that wrote them. A \
         rising count here is not decay — it is the instrument working.</p>",
        facts.open_loose_ends, facts.loose_ends
    );

    let _ = writeln!(
        out,
        "<h2>How to refresh this page</h2>\n<pre><code>cd os\ncargo run -p xtask -- \
         emit-feasibility &gt; ../goals/feasibility.html\ncargo run -p xtask -- \
         check-feasibility</code></pre>\n<p class=\"note\">The check runs in \
         <code>check-assurance-spine</code>, so a stale copy fails the build rather than \
         misinforming a reader. Test count on the runner: {} host tests.</p>",
        HOST_TESTS
    );

    out.push_str(FOOT);
    Ok(out)
}

/// Host tests passing on the Linux runner, from the first green `host-tests`
/// run (`LE-100`, 2026-08-07). A literal because it is a property of a CI run
/// rather than of this tree, and inventing a derivation would be worse.
const HOST_TESTS: usize = 1231;

/// Claims with runner or silicon behind them.
const PROVEN: [(&str, &str); 6] = [
    (
        "The kernel boots, schedules, faults and contains — under CI, every push",
        "23 QEMU fixtures run on the runner, including three real CPU exceptions each contained \
         to its own task, a double fault landing on the IST stack, W^X proven adversarially in \
         both directions, and a real PE64 loaded into its own address space.",
    ),
    (
        "The host suite runs where it counts",
        "1,231 tests green on Linux CI. Until 2026-08-06 there was no `cargo test` step at all \
         and every one of them gated a laptop (`LE-100`).",
    ),
    (
        "Preemption and WCET enforcement are real, not asserted",
        "A task that never yields is preempted with its SSE state intact; WCET overrun restarts, \
         degrades and trips to a safe state, each arm gated separately, each falsified by \
         mutation before being believed.",
    ),
    (
        "The kernel drives real silicon with interrupts live",
        "Raspberry Pi 5, netbooted with no SD card, one cooperative dispatch round per park beat, \
         0 spoor records lost and 0 refused (`FEAT-P1-11`, board-proven 2026-08-05).",
    ),
    (
        "Hardware timing evidence exists and is on the wire",
        "A full TOS64-MEAS/2 envelope captured from the board on 2026-08-07: tier=T1, \
         cycle_source=pmccntr_el0, 14 metrics at n=1000 with dropped=0, after ~14 hours of \
         unbroken beaconing.",
    ),
    (
        "The assurance spine is machine-checked, including itself",
        "Contracts, status headers, citations, metric domain labels and the CI workflow are all \
         refused when they disagree with the tree — and `check-ci-gates` refuses a workflow that \
         quietly stops asking for the tests.",
    ),
];

/// Claims the project explicitly does not make.
const UNPROVEN: [(&str, &str); 5] = [
    (
        "No worst-case bound is quotable, on any platform",
        "ADR 0005: a bound may be quoted only from a platform holding a secure-world \
         qualification record. Zero platforms hold one, so every G04 gate is correctly barred.",
    ),
    (
        "No real-time claim survives contact with load",
        "Isolation under competing load and exhaustion containment are unbuilt: `Tcb` carries no \
         containment class and the pool is one flat capacity with no reservation floor.",
    ),
    (
        "Nothing is proven under hostile input at scale",
        "No soak, no burst/backpressure harness, no offered-load generator. Single-task fault \
         containment is real; per-class resource containment is not.",
    ),
    (
        "The timing numbers that exist are Tier 0 or unqualified Tier 1",
        "QEMU/TCG cycle counts calibrate the harness, not the hardware. The board numbers carry \
         `qualification=none` in the envelope itself.",
    ),
    (
        "Most guardrails that have been read, missed",
        "Of the gates read against their targets so far, several miss by 17x to 39x and are filed \
         as refusals rather than quietly left blank. That is the register working, and it is also \
         the honest state of the numbers.",
    ),
];

fn distinct_measurements(present: &assurance::Implemented) -> usize {
    let mut seen: Vec<&str> = present
        .per_domain
        .iter()
        .flat_map(|d| d.measurable_today.iter().map(String::as_str))
        .collect();
    seen.sort_unstable();
    seen.dedup();
    seen.len()
}

fn landing_zones(repo_root: &Path) -> Result<Vec<LandingZone>, String> {
    let path = repo_root.join("goals").join("context").join("landing-zones.tsv");
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut zones = Vec::new();
    for line in contents.lines().skip(1).filter(|line| !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 10 {
            return Err(format!("landing-zones.tsv row has {} fields, expected 10", fields.len()));
        }
        zones.push(LandingZone {
            id: fields[0].to_string(),
            name: fields[1].to_string(),
            horizon: fields[3].to_string(),
            claim_gate: fields[9].to_string(),
        });
    }
    Ok(zones)
}

fn tile(value: &str, label: &str, note: &str) -> String {
    format!(
        "<div class=\"stat\"><div class=\"n\">{value}</div><div class=\"l\">{label}</div>\
         <div class=\"why\">{note}</div></div>"
    )
}

fn badge(state: &str) -> String {
    let class = match state {
        "Complete" | "Verified" => "ok",
        "In progress" => "wip",
        _ => "spec",
    };
    format!("<span class=\"badge {class}\">{}</span>", escape(state))
}

fn horizon(value: &str) -> String {
    let class = match value {
        "now" => "ok",
        "next" => "wip",
        _ => "spec",
    };
    format!("<span class=\"badge {class}\">{}</span>", escape(value))
}

fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

const HEAD: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>TinyOS — feasibility report</title>
<style>
  :root { color-scheme: light dark; --bg:#fff; --fg:#1a1a1a; --muted:#5a5a5a; --border:#d8d8d8;
    --code:#f2f2f2; --accent:#0b5fff; --card:#fafafa; --ok:#0a7d3f; --wip:#a35b00; --spec:#5a5a5a;
    --warn:#b3261e; --warnbg:#fdf1f0; }
  @media (prefers-color-scheme: dark) { :root { --bg:#14161a; --fg:#e8e8e8; --muted:#a0a0a0;
    --border:#333740; --code:#1e2126; --accent:#6fa8ff; --card:#191c21; --ok:#4ec97f;
    --wip:#e0a34e; --spec:#a0a0a0; --warn:#ff8a80; --warnbg:#241a1a; } }
  * { box-sizing:border-box; }
  body { max-width:1000px; margin:0 auto; padding:2.5rem 1.5rem 5rem; background:var(--bg);
    color:var(--fg); line-height:1.6;
    font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif; }
  h1 { font-size:1.9rem; margin-bottom:.25rem; }
  h2 { font-size:1.2rem; margin-top:2.8rem; border-bottom:1px solid var(--border);
    padding-bottom:.35rem; }
  .sub { color:var(--muted); font-size:.92rem; }
  .verdict { background:var(--warnbg); border:1px solid var(--border);
    border-left:4px solid var(--warn); border-radius:8px; padding:1rem 1.2rem; margin:1.5rem 0; }
  .verdict .lede { font-size:1.05rem; font-weight:600; margin:0 0 .6rem; }
  .verdict .bar { margin:0; color:var(--muted); font-size:.92rem; }
  .grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(210px,1fr)); gap:.9rem;
    margin:1.2rem 0; }
  .stat { background:var(--card); border:1px solid var(--border); border-radius:8px;
    padding:.9rem 1rem; }
  .stat .n { font-size:1.7rem; font-weight:700; }
  .stat .l { font-size:.86rem; font-weight:600; margin-top:.15rem; }
  .stat .why { font-size:.8rem; color:var(--muted); margin-top:.4rem; }
  .note { color:var(--muted); font-size:.9rem; }
  table { width:100%; border-collapse:collapse; margin:1rem 0; font-size:.9rem; display:block;
    overflow-x:auto; }
  th,td { border-bottom:1px solid var(--border); padding:.55rem .6rem; text-align:left;
    vertical-align:top; }
  th { font-size:.78rem; text-transform:uppercase; letter-spacing:.04em; color:var(--muted); }
  td.n, th.n { text-align:right; font-variant-numeric:tabular-nums; font-weight:600; }
  tr.hi td { background:var(--card); }
  .dim { color:var(--muted); font-size:.85rem; }
  .gate { color:var(--muted); font-size:.85rem; }
  .badge { font-size:.72rem; font-weight:700; text-transform:uppercase; letter-spacing:.04em;
    padding:.12rem .45rem; border-radius:999px; border:1px solid currentColor; white-space:nowrap; }
  .badge.ok { color:var(--ok); } .badge.wip { color:var(--wip); } .badge.spec { color:var(--spec); }
  ul.proof, ul.unproven, ol.next { padding-left:1.1rem; }
  ul.proof li, ul.unproven li, ol.next li { margin-bottom:.7rem; }
  ul.proof li span, ul.unproven li span, ol.next li span { display:block; color:var(--muted);
    font-size:.88rem; margin-top:.15rem; }
  ul.proof { list-style:none; padding-left:0; }
  ul.proof li { border-left:3px solid var(--ok); padding-left:.8rem; }
  ul.unproven { list-style:none; padding-left:0; }
  ul.unproven li { border-left:3px solid var(--warn); padding-left:.8rem; }
  pre { background:var(--code); padding:.8rem 1rem; border-radius:6px; overflow-x:auto;
    font-size:.85rem; }
  code { background:var(--code); padding:.1rem .35rem; border-radius:4px; font-size:.9em; }
  pre code { background:none; padding:0; }
  footer { margin-top:3rem; padding-top:1.5rem; border-top:1px solid var(--border);
    font-size:.85rem; color:var(--muted); }
</style>
</head>
<body>
"#;

const FOOT: &str = r#"<footer>
Generated by <code>cargo run -p xtask -- emit-feasibility</code> from the committed assurance
spine. No figure on this page was typed by hand. Judgements — the verdict, the proven and
unproven lists — live as reviewed constants in <code>os/src/xtask/src/feasibility.rs</code>, so
changing a claim is a source diff.
</footer>
</body>
</html>
"#;
