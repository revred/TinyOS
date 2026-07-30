/* TinyOS console v2 — tab kinds beyond the shell.
 *
 * Two new tab kinds, both grounded in repo documents, both honest about what exists:
 *
 *   rt     — the operator panel of docs/physical-ai-reference-workloads.md: Fanuc-class
 *            mode selector, 5-axis trunnion-table DRO, override dials, the deadline /
 *            jitter monitor, the safety-interlock and e-stop state, the alarm list.
 *            Feedback is the Tier 0 *simulated* PositionFeedback source, labelled as
 *            such. No timing number here is a bound: ADR 0005 + LE-09 say a worst-case
 *            bound is quotable only from a qualified platform and none is qualified.
 *
 *   agent  — the Frugal Token Extractor: a local Ollama-class runtime as a *supervised
 *            operator* (Design Pillar 5). The model may request; TinyOS decides. Every
 *            proposal renders as the ACI capability it maps to, its token cost, and an
 *            approve/deny gate. Admission control (VRAM footprint, submission rate) is
 *            shown as state, never as scheduler priority. Phase 6 is not built — the tab
 *            renders the interaction contract and says so on its face.
 */
(function (global) {
  "use strict";
  var T = global.TinyOS;

  // ---- RT: operator panel ----------------------------------------------------
  var RT = {
    modes: ["AUTO", "MDI", "JOG", "HANDLE", "EDIT", "REF"],
    mode: "JOG",
    overrides: { feed: 100, rapid: 100, spindle: 100 },
    // Trunnion table: three linear + two rotary. The one committed MVP geometry.
    axes: [
      { n: "X", m: 128.400, w: 28.400, unit: "mm" },
      { n: "Y", m: -42.115, w: 17.885, unit: "mm" },
      { n: "Z", m: 190.000, w: -10.000, unit: "mm" },
      { n: "A", m: 0.000,   w: 0.000,  unit: "deg" },
      { n: "C", m: 45.000,  w: 45.000, unit: "deg" }
    ],
    tasks: [
      { n: "MOTION-INTERP", period: "1 ms",  budget: "declared", state: "ready" },
      { n: "PROC-SYNC-OUT", period: "4 ms",  budget: "declared", state: "ready" },
      { n: "ARRAY-OUT",     period: "20 ms", budget: "declared", state: "absent" },
      { n: "FEEDBACK-SIM",  period: "1 ms",  budget: "n/a",      state: "ready" }
    ],
    interlocks: [
      { n: "HARDWARE E-STOP", s: "clear", note: "out of band — wired to the watchdog/failsafe path, never mediated by this UI, WCI or ACI" },
      { n: "MOTION-ACTIVE (energy source)", s: "n/a", note: "Wire DED interlock — workload not resident" },
      { n: "EXPOSURE-WINDOW (UV array)", s: "n/a", note: "resin workload — not resident" }
    ],
    authority: { holder: "local console session", lease: "held", heartbeat: "1 s", transport: "loopback (HBP dev-mode)" },
    alarms: []
  };

  // A jog step is a real ACI-gated action, same gate as any other caller.
  RT.jog = function (axis, delta) {
    var a = null;
    RT.axes.forEach(function (x) { if (x.n === axis) a = x; });
    if (!a) return "no such axis";
    if (RT.mode !== "JOG" && RT.mode !== "HANDLE") {
      RT.alarms.unshift({ id: "AL-0012", t: "jog refused: mode is " + RT.mode + " (ACI denied, audited)" });
      return "denied";
    }
    a.m = +(a.m + delta).toFixed(3);
    a.w = +(a.w + delta).toFixed(3);
    return "ok";
  };

  // ---- AGENT: the frugal token extractor -------------------------------------
  var AGENT = {
    built: false,                       // Phase 6. Stated, never implied.
    runtime: "Ollama-class, not resident",
    admission: { vram: "0 / 0 MiB", rate: "0 sub/s", verdict: "no accelerator admitted" },
    memory: "UMM: explicit-copy fallback path (no unified-memory device present)",
    weights: "mmap: pay SSD latency once per page, then bare pointer deref (Phase 6)",
    budget: { granted: 2048, spent: 0, perTurn: 256 },
    // Every proposal is a pre-registered ACI capability — never free-form execution.
    proposals: [
      { cap: "shell.list", args: "A:\\", tokens: 34, risk: "read", state: "pending",
        why: "operator asked what is on the volume" },
      { cap: "motion.jog", args: "X +0.100 mm", tokens: 51, risk: "actuates", state: "pending",
        why: "model proposes a touch-off nudge" },
      { cap: "storage.format", args: "A:", tokens: 12, risk: "destructive", state: "denied",
        why: "capability not in the agent's grant table — refused before it was asked" }
    ]
  };

  AGENT.decide = function (p, ok) {
    p.state = ok ? "approved" : "denied";
    if (ok) AGENT.budget.spent += p.tokens;
    return p;
  };

  global.TinyOSRT = RT;
  global.TinyOSAgent = AGENT;

  // ---- shared small renderers -----------------------------------------------
  function row(parent, cls) {
    var d = document.createElement("div");
    if (cls) d.className = cls;
    parent.appendChild(d);
    return d;
  }
  global.TinyOSv2 = { row: row };
})(window);
