/* TinyOS console — tab bodies, shared by every layout.
 *
 * One renderer per tab kind, plus one "context column" per kind. A layout decides where
 * these go (full-bleed, beside a live column, or two at once); it never decides what they
 * say. Fixed by preference and not a layout choice: the system line is the last line of
 * the window, under the prompt, and carries no label about itself.
 */
(function (global) {
  "use strict";
  var T = global.TinyOS, RT = global.TinyOSRT, AG = global.TinyOSAgent;

  function el(tag, cls, html) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html) e.innerHTML = html;
    return e;
  }
  function kv(parent, k, v, colour, title) {
    var a = el("span", "k"); a.textContent = k; if (title) a.title = title;
    var b = el("span"); b.textContent = v; if (colour) b.style.color = colour;
    parent.append(a, b);
  }

  var state = { parity: "not run" };

  // ---- shell ----------------------------------------------------------------
  function shellBody(s) {
    var sc = el("div", "screen");
    sc.tabIndex = 0; sc.setAttribute("aria-live", "polite");
    T.mountTranscript(sc, s);
    return sc;
  }

  // ---- rt -------------------------------------------------------------------
  function rtBody(s, ctx) {
    var wrap = el("div", "pad");
    wrap.append(el("h4", null, "Mode — operator panel"));
    var modes = el("div", "modes");
    RT.modes.forEach(function (mo) {
      var b = el("button"); b.textContent = mo; b.setAttribute("aria-pressed", RT.mode === mo);
      b.onclick = function () { RT.mode = mo; ctx.repaint(); };
      modes.appendChild(b);
    });
    T.rove(modes, "button");
    wrap.append(modes, el("h4", null, "Position — trunnion table (X Y Z A C)"));

    var dro = el("div", "dro");
    ["", "MACHINE", "WORK", ""].forEach(function (h) {
      var e = el("span", "h"); e.textContent = h; dro.appendChild(e);
    });
    RT.axes.forEach(function (a) {
      var ax = el("span", "ax"); ax.textContent = a.n;
      var mv = el("span", "num"); mv.textContent = a.m.toFixed(3);
      var wv = el("span", "num w"); wv.textContent = a.w.toFixed(3);
      var j = el("span", "jog");
      [["−", -0.1], ["+", 0.1]].forEach(function (p) {
        var b = el("button"); b.textContent = p[0];
        b.title = "JOG " + a.n + " " + (p[1] > 0 ? "+" : "") + p[1];
        b.onclick = function () { ctx.jog(a.n, p[1]); };
        j.appendChild(b);
      });
      dro.append(ax, mv, wv, j);
    });
    wrap.append(dro, el("p", "caveat",
      "Feedback source: <b>simulated</b> PositionFeedback (Tier 0). Real encoders bolt on " +
      "against the same trait — positional accuracy is not validated here and is not claimed."));

    wrap.append(el("h4", null, "Overrides"));
    var ovr = el("div", "ovr");
    Object.keys(RT.overrides).forEach(function (k) {
      var lab = el("span"); lab.textContent = k.toUpperCase();
      var r = document.createElement("input");
      r.type = "range"; r.min = 0; r.max = 150; r.step = 5; r.value = RT.overrides[k];
      r.setAttribute("aria-label", k + " override percent");
      var v = el("span"); v.textContent = RT.overrides[k] + "%";
      r.oninput = function () { RT.overrides[k] = +r.value; v.textContent = r.value + "%"; };
      ovr.append(lab, r, v);
    });
    wrap.appendChild(ovr);
    return wrap;
  }

  function rtContext() {
    var w = el("div", "pad");
    w.append(el("h4", null, "Deadline &amp; jitter monitor"));
    var t = el("div", "kv");
    RT.tasks.forEach(function (x) {
      kv(t, x.n + " · " + x.period, x.state === "absent" ? "not resident" : "budget " + x.budget,
        x.state === "absent" ? "var(--t-absent)" : "var(--t-pending)");
    });
    w.append(t, el("p", "caveat",
      "Mechanism evidence only. A worst-case bound is quotable solely from a <b>qualified</b> " +
      "ARM64 platform (ADR 0005); none is qualified (LE-09) and no ARM64 code here has executed (LE-27)."));

    w.append(el("h4", null, "Safety interlocks"));
    var i = el("div", "kv");
    RT.interlocks.forEach(function (x) {
      kv(i, x.n, x.s, x.s === "clear" ? "var(--t-pass)" : "var(--t-absent)", x.note);
    });
    w.append(i, el("div", "estop",
      "<b>E-STOP</b><span class='l-meta'>hard-wired to the watchdog/failsafe path — never " +
      "mediated by this console, by WCI, or by the ACI</span>"));

    w.append(el("h4", null, "Command authority (single writer)"));
    var a = el("div", "kv");
    kv(a, "holder", RT.authority.holder); kv(a, "lease", RT.authority.lease, "var(--t-pass)");
    kv(a, "heartbeat", RT.authority.heartbeat); kv(a, "transport", RT.authority.transport);
    w.appendChild(a);

    w.append(el("h4", null, "Alarms"));
    var al = el("div", "kv");
    if (!RT.alarms.length) kv(al, "no active alarms", "");
    else RT.alarms.forEach(function (x) { kv(al, x.id, x.t, "var(--t-err)"); });
    w.appendChild(al);
    return w;
  }

  // ---- agent ----------------------------------------------------------------
  function agentBody(s, ctx) {
    var w = el("div", "pad");
    w.append(el("h4", null, "Proposals — the model requests, TinyOS decides"));
    AG.proposals.forEach(function (p) {
      var d = el("div", "prop"); d.dataset.state = p.state;
      d.innerHTML = "<div><span class='cap'></span> <span class='risk'></span></div>" +
        "<div class='why'></div><div class='acts'></div>";
      d.querySelector(".cap").textContent = p.cap + "(" + p.args + ")";
      var r = d.querySelector(".risk"); r.textContent = p.risk; r.dataset.r = p.risk;
      d.querySelector(".why").textContent = p.why;
      var acts = d.querySelector(".acts");
      if (p.state === "pending") {
        [["Approve", true], ["Deny", false]].forEach(function (pair) {
          var b = el("button", "chip"); b.textContent = pair[0];
          b.onclick = function () {
            AG.decide(p, pair[1]);
            s.emit(pair[1] ? "out" : "deny",
              (pair[1] ? "APPROVED " : "DENIED ") + p.cap + " — " + p.tokens +
              " tokens charged to the agent budget, provenance logged");
            ctx.repaint();
          };
          acts.appendChild(b);
        });
      } else {
        var st = el("span", p.state === "approved" ? "l-pass" : "l-deny");
        st.textContent = p.state === "denied" && p.risk === "destructive"
          ? "refused at the grant table — the capability was never offered" : p.state;
        acts.appendChild(st);
      }
      var tk = el("span", "tok"); tk.textContent = p.tokens + " tok"; acts.appendChild(tk);
      w.appendChild(d);
    });
    return w;
  }

  function agentContext() {
    var pct = Math.min(100, Math.round(AG.budget.spent / AG.budget.granted * 100));
    var w = el("div", "pad");
    w.append(el("h4", null, "Token budget — frugal by construction"));
    var b = el("div", "kv");
    kv(b, "granted", String(AG.budget.granted));
    kv(b, "spent", String(AG.budget.spent), AG.budget.spent ? "var(--t-pending)" : null);
    kv(b, "per-turn cap", String(AG.budget.perTurn));
    w.append(b, el("div", "bar", "<i style='width:" + pct + "%'></i>"));

    w.append(el("h4", null, "Admission control"));
    var a = el("div", "kv");
    kv(a, "runtime", AG.runtime, "var(--t-absent)");
    kv(a, "VRAM", AG.admission.vram); kv(a, "submission rate", AG.admission.rate);
    kv(a, "verdict", AG.admission.verdict, "var(--t-absent)");
    w.append(a, el("p", "caveat",
      "Admission-controlled, never scheduler-privileged: a stalled model degrades or errors " +
      "through the ACI and cannot delay an RT deadline (Non-Negotiable 6)."));

    w.append(el("h4", null, "Memory &amp; weights"));
    var m = el("div", "kv");
    kv(m, "UMM", AG.memory); kv(m, "weights", AG.weights);
    w.append(m, el("p", "caveat",
      "Phase 6 is not built. This is the interaction contract — the gate, the budget and the " +
      "provenance — rendered ahead of the runtime so the runtime cannot arrive without them."));
    return w;
  }

  // ---- parity ---------------------------------------------------------------
  function parityBody() {
    var w = el("div", "pad");
    w.append(el("h4", null, "MS-DOS parity suite — three-signal rule"));
    var g = el("div", "kv"), green = state.parity === "PASS";
    ["fixture isa-debug-exit", "transcript vs golden", "spoor journal corroborates", "OVERALL"]
      .forEach(function (n, i) {
        kv(g, n, green ? (i === 3 ? "PASS" : "green")
          : state.parity === "running…" ? "running…" : "no signal yet",
          green ? "var(--t-pass)" : "var(--t-absent)");
      });
    w.append(g, el("p", "caveat",
      "Aggregated affirmative-all-or-FAIL. Signal 1 is the fixture's in-guest exit verdict under " +
      "QEMU, signal 2 the 64-line golden byte comparison, signal 3 (LE-56) the spoor journal " +
      "corroborating the denial count. Press F8."));
    return w;
  }

  // ---- evidence context for a shell tab -------------------------------------
  var SPINE = [
    ["FEAT-P2-04", "DOS front-end + .TCB runtime", "verified", "live"],
    ["STORY-P2-07-02", "spoor journal as third signal", "verified", "live"],
    ["FEAT-P2-05", "POSIX front-end + 3-way equivalence", "queued", "absent"],
    ["EPIC-P1", "determinism proof", "in progress", "pending"],
    ["LE-09", "no qualified platform — bounds not quotable", "open", "absent"],
    ["LE-53", "host-side only, not the on-target tab host", "open", "absent"]
  ];
  function shellContext(s) {
    var w = el("div", "pad");
    w.append(el("h4", null, "Session"));
    var a = el("div", "kv");
    kv(a, "identity", s.session); kv(a, "flavour", T.FLAVOURS[s.flavour].label);
    kv(a, "cwd", "A:" + s.cwd); kv(a, "volume", s.vol.label + " " + s.vol.serial);
    kv(a, "denials", String(s.spoors.length), s.spoors.length ? "var(--t-deny)" : "var(--t-pass)");
    w.appendChild(a);

    w.append(el("h4", null, "Authority ledger"));
    var l = el("div", "kv");
    if (!s.spoors.length) kv(l, "no authority denials", "");
    else s.spoors.forEach(function (sp, i) {
      kv(l, (i + 1) + " " + sp.act, sp.target, "var(--t-deny)");
    });
    w.appendChild(l);

    w.append(el("h4", null, "Goals spine"));
    var g = el("div", "kv");
    SPINE.forEach(function (r) {
      kv(g, r[0] + " · " + r[1], r[2],
        r[3] === "live" ? "var(--t-pass)" : r[3] === "pending" ? "var(--t-pending)" : "var(--t-absent)");
    });
    w.appendChild(g);
    return w;
  }

  global.TinyOSBodies = {
    state: state,
    body: function (s, ctx) {
      return s.tkind === "rt" ? rtBody(s, ctx)
           : s.tkind === "agent" ? agentBody(s, ctx)
           : s.tkind === "parity" ? parityBody()
           : shellBody(s);
    },
    context: function (s) {
      return s.tkind === "rt" ? rtContext()
           : s.tkind === "agent" ? agentContext()
           : s.tkind === "parity" ? rtContext()
           : shellContext(s);
    },
    contextTitle: function (s) {
      return s.tkind === "rt" ? "MACHINE STATE"
           : s.tkind === "agent" ? "BUDGET & ADMISSION"
           : s.tkind === "parity" ? "MACHINE STATE" : "SESSION & EVIDENCE";
    }
  };
})(window);
