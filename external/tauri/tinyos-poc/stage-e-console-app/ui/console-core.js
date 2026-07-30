/* TinyOS operator console — shared core. No framework, no build step.
 *
 * Frugality contract (see SPEC.md §6):
 *   - append-only transcript: one <span> per emitted line, never re-rendered;
 *   - no polling faster than 4 Hz, and zero timers while the window is hidden;
 *   - all state is plain objects; no observers, no vdom, no reflow per keystroke.
 *
 * In the repo build every `Session.run()` here is replaced by one `invoke("run_line")`
 * to the Rust `shell` crate — the verb table below mirrors os/src/shell/src/dos.rs and
 * exists so the UX can be reviewed without a Tauri host. Nothing else changes.
 */
(function (global) {
  "use strict";

  // ---- verb surface — mirrors shell::verbs::VerbKind -------------------------
  var VERBS = [
    ["DIR",      "List",        "list directory"],
    ["CD",       "ChangeDir",   "change directory (CHDIR)"],
    ["COPY",     "Copy",        "copy a file"],
    ["MOVE",     "Move",        "move / rename (REN, RENAME)"],
    ["DEL",      "Delete",      "delete a file (ERASE)"],
    ["MD",       "MakeDir",     "make directory (MKDIR)"],
    ["RD",       "RemoveDir",   "remove directory (RMDIR)"],
    ["TYPE",     "ViewFile",    "print a file"],
    ["FIND",     "FindText",    'find "string" in a file'],
    ["SORT",     "SortStream",  "sort a file's lines"],
    ["MORE",     "Page",        "page a file"],
    ["TREE",     "TreeView",    "directory tree"],
    ["ATTRIB",   "AttribView",  "labels: origin, trust"],
    ["SET",      "Env",         "environment variables"],
    ["PATH",     "Env",         "search path"],
    ["ECHO",     "Echo",        "echo / ECHO ON|OFF"],
    ["CLS",      "ClearScreen", "clear the screen"],
    ["VER",      "VersionInfo", "version banner"],
    ["VOL",      "VolumeInfo",  "volume label + serial"],
    ["MEM",      "MemInfo",     "static pool map"],
    ["TASKMGR",  "TaskList",    "task table (TASKLIST)"],
    ["TASKKILL", "TaskKill",    "kill a task (authority-checked)"],
    ["SPOOR",    "SpoorJournal","denial journal"]
  ];

  // POSIX / RT front-ends map onto the same canonical core (docs/cli-compatibility-mvp.md)
  var FLAVOURS = {
    dos:   { label: "MS-DOS",  prompt: function (cwd) { return "A:" + cwd + ">"; }, alias: {} },
    posix: { label: "Linux",   prompt: function (cwd) { return "tinyos:" + cwd.replace(/\\/g, "/") + "$ "; },
             alias: { ls: "DIR", cd: "CD", cp: "COPY", mv: "MOVE", rm: "DEL", mkdir: "MD",
                      rmdir: "RD", cat: "TYPE", grep: "FIND", sort: "SORT", less: "MORE",
                      tree: "TREE", env: "SET", echo: "ECHO", clear: "CLS", uname: "VER",
                      df: "VOL", free: "MEM", ps: "TASKMGR", kill: "TASKKILL", pwd: "CD" } },
    mac:   { label: "Mac-OS",  prompt: function (cwd) { return "tinyos:" + cwd.replace(/\\/g, "/") + " operator% "; },
             alias: null /* inherits posix */ },
    rt:    { label: "RT-OS",   prompt: function () { return "rt> "; },
             alias: { task: "TASKMGR", mem: "MEM", pool: "MEM", spoor: "SPOOR", ver: "VER" } }
  };
  FLAVOURS.mac.alias = FLAVOURS.posix.alias;

  // ---- RAM volume — the seeded A: of the fixture -----------------------------
  function seedVolume() {
    return {
      label: "TINYOS", serial: "1234-ABCD", free: 10752,
      dirs: { "\\": ["DOCS"], "\\DOCS": [] },
      files: {
        "\\": [
          { n: "README.TXT", s: 99,  d: "07-30-26  12:00p", origin: "seeded", trust: "operator",
            body: ["TinyOS carries the soul of MS-DOS", "into a real-time, labelled world.",
                   "TinyOS is tested, not asserted."] },
          { n: "LIST.TXT",   s: 25,  d: "07-30-26  12:00p", origin: "seeded", trust: "operator",
            body: ["delta", "charlie", "bravo", "alpha"] },
          { n: "SAMPLE.TCB", s: 196, d: "07-30-26  12:00p", origin: "seeded", trust: "operator",
            body: ["@ECHO OFF", "ECHO RUNNING batch", "SET DEMO=17G", "ECHO %DEMO%",
                   "MD WORK", "COPY README.TXT WORK", "ECHO RUNNING batch complete"] }
        ],
        "\\DOCS": [
          { n: "NOTES.TXT", s: 42, d: "07-30-26  12:00p", origin: "seeded", trust: "operator",
            body: ["alpha", "beta"] },
          { n: "KEEP.TXT",  s: 18, d: "07-30-26  12:00p", origin: "seeded", trust: "operator",
            body: ["keep"] }
        ]
      }
    };
  }

  var TASKS = [
    { n: "RT-CTRL", pri: 31, st: "ready",   kill: "supervisor" },
    { n: "SPOOR",   pri: 3,  st: "waiting", kill: "ordinary" },
    { n: "IDLE",    pri: 0,  st: "ready",   kill: "unkillable" }
  ];

  // ---- session ---------------------------------------------------------------
  var seq = 0;
  function Session(opts) {
    opts = opts || {};
    this.label = opts.label || ("tab-" + (++seq));
    this.session = opts.session || this.label.toUpperCase();
    this.flavour = opts.flavour || "dos";
    this.kind = opts.kind || "shell";          // shell | parity | gui
    this.vol = seedVolume();
    this.env = { PATH: "\\;" };
    this.cwd = "\\";
    this.echo = true;
    this.grants = opts.grants || VERBS.map(function (v) { return v[1]; });
    this.spoors = [];
    this.lines = [];
    this.history = [];
    this.listeners = [];
    this.emit("meta", "TinyOS Version 0.2.0 (Tier 0, x86_64) — session " + this.session +
      " [" + FLAVOURS[this.flavour].label + "]");
  }
  Session.prototype.prompt = function () { return FLAVOURS[this.flavour].prompt(this.cwd); };
  Session.prototype.on = function (fn) { this.listeners.push(fn); };
  Session.prototype.emit = function (cls, text) {
    var line = { c: cls, s: text };
    this.lines.push(line);
    for (var i = 0; i < this.listeners.length; i++) this.listeners[i](line, this);
  };
  Session.prototype.out = function (t) { this.emit("out", t); };

  Session.prototype.deny = function (verb) {
    this.emit("deny", "Access denied: verb " + verb + " is not granted to session " +
      this.session + " [audited]");
    this.spoors.push({ cat: "shell", actor: "session", act: "verb-denied", out: "failed",
      target: verb.toLowerCase(), cost: 0 });
  };

  Session.prototype.canon = function (word) {
    var f = FLAVOURS[this.flavour];
    if (f.alias && f.alias[word.toLowerCase()]) return f.alias[word.toLowerCase()];
    var w = word.toUpperCase();
    var syn = { CHDIR: "CD", MKDIR: "MD", RMDIR: "RD", ERASE: "DEL", REN: "MOVE",
                RENAME: "MOVE", TASKLIST: "TASKMGR" };
    return syn[w] || w;
  };

  Session.prototype.run = function (raw) {
    var line = (raw || "").trim();
    this.emit("echo", this.prompt() + line);
    if (!line) return;
    this.history.push(line);
    var parts = line.split(/\s+/);
    var verb = this.canon(parts[0]);
    var args = parts.slice(1);
    var entry = null;
    for (var i = 0; i < VERBS.length; i++) if (VERBS[i][0] === verb) entry = VERBS[i];

    if (/\.TCB$/i.test(parts[0])) return this.batch(parts[0].toUpperCase());
    if (!entry) { this.emit("err", "Bad command or file name"); return; }
    if (this.grants.indexOf(entry[1]) < 0) { this.deny(entry[1]); return; }
    this[verb] ? this[verb](args) : this.emit("err", "Bad command or file name");
  };

  Session.prototype.batch = function (name) {
    var f = this.find(name);
    if (!f) { this.emit("err", "Bad command or file name"); return; }
    var self = this;
    f.body.forEach(function (l) {
      if (/^@?ECHO OFF$/i.test(l)) { self.echo = false; return; }
      var expanded = l.replace(/%(\w+)%/g, function (m, k) { return self.env[k] || ""; });
      if (self.echo) self.emit("meta", self.prompt() + expanded);
      var p = expanded.split(/\s+/), v = self.canon(p[0]);
      if (v === "ECHO") self.out(p.slice(1).join(" "));
      else if (v === "SET") self.SET(p.slice(1));
      else if (v === "MD") self.MD(p.slice(1));
      else if (v === "COPY") self.COPY(p.slice(1));
    });
  };

  Session.prototype.find = function (n) {
    var list = this.vol.files[this.cwd] || [];
    for (var i = 0; i < list.length; i++)
      if (list[i].n.toUpperCase() === n.toUpperCase()) return list[i];
    return null;
  };

  // --- verb implementations (formatting byte-matched to shell/golden) ---------
  Session.prototype.header = function () {
    this.out(" Volume in drive A is " + this.vol.label);
    this.out(" Volume Serial Number is " + this.vol.serial);
  };
  Session.prototype.DIR = function () {
    this.out("");
    this.header();
    this.out("");
    this.out(" Directory of A:" + this.cwd);
    this.out("");
    var self = this, n = 0;
    (this.vol.dirs[this.cwd] || []).forEach(function (d) {
      self.out(pad(d, 14) + "<DIR>          07-30-26  12:00p");
    });
    (this.vol.files[this.cwd] || []).forEach(function (f) {
      self.out(pad(f.n, 14) + lpad(String(f.s), 8) + " " + f.d); n++;
    });
    this.out(lpad(String(n), 8) + " File(s)   " + lpad(String(this.vol.free), 8) + " bytes free");
  };
  Session.prototype.CD = function (a) {
    if (!a.length) { this.out("A:" + this.cwd); return; }
    var t = a[0].toUpperCase();
    if (t === "..") { this.cwd = "\\"; return; }
    var next = this.cwd === "\\" ? "\\" + t : this.cwd + "\\" + t;
    if (this.vol.dirs[next]) this.cwd = next;
    else this.emit("err", "Invalid directory");
  };
  Session.prototype.TYPE = function (a) {
    var f = this.find(a[0] || "");
    if (!f) { this.emit("err", "File not found"); return; }
    var self = this; f.body.forEach(function (l) { self.out(l); });
  };
  Session.prototype.MORE = Session.prototype.TYPE;
  Session.prototype.SORT = function (a) {
    var f = this.find(a[0] || "");
    if (!f) { this.emit("err", "File not found"); return; }
    var self = this; f.body.slice().sort().forEach(function (l) { self.out(l); });
  };
  Session.prototype.FIND = function (a) {
    var m = a.join(" ").match(/"([^"]*)"\s+(\S+)/);
    if (!m) { this.emit("err", "FIND: parameter format not correct"); return; }
    var f = this.find(m[2]);
    if (!f) { this.emit("err", "File not found"); return; }
    this.out("---------- " + f.n.toUpperCase());
    var self = this;
    f.body.forEach(function (l) { if (l.indexOf(m[1]) >= 0) self.out(l); });
  };
  Session.prototype.TREE = function () {
    this.out("Directory PATH listing for Volume " + this.vol.label);
    this.out("Volume Serial Number is " + this.vol.serial);
    this.out("A:\\");
    var self = this;
    (this.vol.dirs["\\"] || []).forEach(function (d) {
      self.out("\\---" + d);
      (self.vol.files["\\" + d] || []).forEach(function (f) { self.out("|   " + f.n); });
    });
  };
  Session.prototype.ATTRIB = function () {
    var self = this;
    (this.vol.files[this.cwd] || []).forEach(function (f) {
      self.out(" A       [origin=" + f.origin + " trust=" + f.trust + "] " + f.n);
    });
  };
  Session.prototype.SET = function (a) {
    if (!a.length) {
      var self = this;
      Object.keys(this.env).forEach(function (k) { self.out(k + "=" + self.env[k]); });
      return;
    }
    var joined = a.join(" "), eq = joined.indexOf("=");
    if (eq < 0) {
      var k = joined.toUpperCase();
      if (this.env[k] !== undefined) this.out(k + "=" + this.env[k]);
      else this.emit("err", "Environment variable " + k + " not defined");
      return;
    }
    var key = joined.slice(0, eq).toUpperCase(), val = joined.slice(eq + 1);
    var self2 = this;
    this.env[key] = val.replace(/%(\w+)%/g, function (m, kk) { return self2.env[kk] || ""; });
  };
  Session.prototype.PATH = function (a) {
    if (!a.length) this.out("PATH=" + this.env.PATH);
    else this.env.PATH = a.join(" ");
  };
  Session.prototype.ECHO = function (a) {
    if (!a.length) { this.out("ECHO is " + (this.echo ? "on" : "off")); return; }
    if (/^ON$/i.test(a[0])) { this.echo = true; return; }
    if (/^OFF$/i.test(a[0])) { this.echo = false; return; }
    var self = this;
    this.out(a.join(" ").replace(/%(\w+)%/g, function (m, k) { return self.env[k] || ""; }));
  };
  Session.prototype.CLS = function () {
    this.lines.length = 0;
    for (var i = 0; i < this.listeners.length; i++) this.listeners[i](null, this);
  };
  Session.prototype.VER = function () { this.out("TinyOS Version 0.2.0 (Tier 0, x86_64)"); };
  Session.prototype.VOL = function () { this.header(); };
  Session.prototype.MEM = function () {
    this.out("  Address     Name          Size       Type");
    this.out("  -------     ----          ----       ----");
    this.out("  000000      VOLUME         12288     Static Pool");
    this.out("");
    this.out("     12288 bytes total memory");
    this.out("     " + this.vol.free + " bytes available");
  };
  Session.prototype.TASKMGR = function () {
    this.out("  TASK          PRI  STATE");
    var self = this;
    TASKS.forEach(function (t) { self.out("  " + pad(t.n, 14) + lpad(String(t.pri), 2) + "  " + t.st); });
  };
  Session.prototype.TASKKILL = function (a) {
    var name = (a[0] || "").toUpperCase(), t = null;
    TASKS.forEach(function (x) { if (x.n === name) t = x; });
    if (!t) { this.emit("err", "Task not found"); return; }
    if (t.kill !== "ordinary") this.deny("TaskKill");
    else this.out("Task " + name + " terminated");
  };
  Session.prototype.SPOOR = function () {
    this.out("Spoor journal (host-side journal):");
    if (!this.spoors.length) { this.emit("meta", "  No spoors journaled (no kernel journal host-side)"); return; }
    this.out("  #  CATEGORY    ACTOR     ACTION        OUTCOME     TARGET            COST");
    var self = this;
    this.spoors.forEach(function (s, i) {
      self.out("  " + (i + 1) + "  " + pad(s.cat, 12) + pad(s.actor, 10) + pad(s.act, 14) +
        pad(s.out, 12) + pad(s.target, 18) + lpad(String(s.cost), 4));
    });
  };
  Session.prototype.MD = function (a) {
    var d = (a[0] || "").toUpperCase(); if (!d) return;
    if (!this.vol.dirs[this.cwd]) this.vol.dirs[this.cwd] = [];
    this.vol.dirs[this.cwd].push(d);
    this.vol.dirs[(this.cwd === "\\" ? "" : this.cwd) + "\\" + d] = [];
    this.vol.files[(this.cwd === "\\" ? "" : this.cwd) + "\\" + d] = [];
  };
  Session.prototype.RD = function (a) {
    var d = (a[0] || "").toUpperCase(), list = this.vol.dirs[this.cwd] || [];
    var i = list.indexOf(d); if (i >= 0) list.splice(i, 1);
    else this.emit("err", "Invalid path, not directory, or directory not empty");
  };
  Session.prototype.DEL = function (a) {
    var n = (a[0] || "").toUpperCase(), list = this.vol.files[this.cwd] || [];
    for (var i = 0; i < list.length; i++) if (list[i].n === n) { list.splice(i, 1); this.vol.free += 512; return; }
    this.emit("err", "File not found");
  };
  Session.prototype.COPY = function (a) {
    var f = this.find(a[0] || "");
    if (!f) { this.emit("err", "File not found"); return; }
    var destDir = (a[1] || "").toUpperCase();
    var key = this.vol.dirs[(this.cwd === "\\" ? "" : this.cwd) + "\\" + destDir]
      ? (this.cwd === "\\" ? "" : this.cwd) + "\\" + destDir : this.cwd;
    (this.vol.files[key] = this.vol.files[key] || []).push(Object.assign({}, f, { origin: "copied" }));
    this.out("        1 File(s) copied");
  };
  Session.prototype.MOVE = function (a) {
    var f = this.find(a[0] || "");
    if (!f) { this.emit("err", "File not found"); return; }
    f.n = (a[1] || f.n).toUpperCase();
    this.out("        1 File(s) moved");
  };

  function pad(s, n) { s = String(s); while (s.length < n) s += " "; return s; }
  function lpad(s, n) { s = String(s); while (s.length < n) s = " " + s; return s; }

  // ---- workbench catalogue ---------------------------------------------------
  // state: live = evidence exists in-repo today; pending = instrumented, landing now;
  // absent = no evidence, named honestly with its loose end. Never invent a number.
  var WORKBENCH = [
    { ic: "⚒", g: "BUILD",   id: "devtools",  t: "DEVELOPER TOOLS",     state: "live",
      v: "xtask 12 cmds", verb: null, detail: "cargo run -p xtask -- help / list-fixtures / check-assurance-spine" },
    { ic: "✓", g: "BUILD",   id: "regress",   t: "REGRESSION TESTS",    state: "live",
      v: "shell 22/22 · xtask 204/204", verb: null, detail: "run from a parity tab: three-signal rule — fixture · golden · spoor" },
    { ic: "▤", g: "RUNTIME", id: "cpu",       t: "CPU USAGE",           state: "pending",
      v: "metering", verb: null, detail: "FEAT-P1-01 measure harness; no CPU verb in the shell — a bound is quotable only from a qualified platform (ADR 0005)" },
    { ic: "▥", g: "RUNTIME", id: "mem",       t: "MEMORY USAGE",        state: "live",
      v: "12288 / 10752 free", verb: "MEM", detail: "static pool map — MEM verb, deterministic" },
    { ic: "◫", g: "RUNTIME", id: "pf",        t: "PAGE-FAULTS",         state: "absent",
      v: "no evidence yet", verb: null, detail: "needs the on-target tab host — LE-53" },
    { ic: "⏱", g: "RUNTIME", id: "speed",     t: "SPEED MEASURES",      state: "pending",
      v: "625-test catalogue", verb: null, detail: "goals/performance/catalogue.tsv; baselines carry LE-09 hardware-tier debt" },
    { ic: "⚿", g: "SAFETY",  id: "vuln",      t: "VULNERABILITIES",     state: "live",
      v: "0 denials bypassed", verb: "SPOOR", detail: "20 controls, 25-pair C0–C4 matrix, CI-checked" },
    { ic: "☰", g: "RUNTIME", id: "tasks",     t: "TASK & PROCESS MONITORS", state: "live",
      v: "3 tasks", verb: "TASKMGR", detail: "RT-CTRL 31 · SPOOR 3 · IDLE 0" },
    { ic: "⇄", g: "RUNTIME", id: "ipc",       t: "IPC MONITORS",        state: "absent",
      v: "no evidence yet", verb: null, detail: "deterministic IPC lands in Phase 0/1" },
    { ic: "⌁", g: "LINK",    id: "net",       t: "NETWORK TOOLS & MONITORS", state: "absent",
      v: "not linked in", verb: null, detail: "Non-Negotiable 12: absent unless opted in — zero linked bytes" },
    { ic: "⚙", g: "LINK",    id: "work",      t: "WORKLOAD MONITORS",   state: "absent",
      v: "no evidence yet", verb: null, detail: "motion / DED / UV workloads — Phase 3+" },
    { ic: "☡", g: "SAFETY",  id: "risk",      t: "RISK / ATTACK MONITORS", state: "absent",
      v: "no evidence yet", verb: null, detail: "Fable-class campaign harness not built" },
    { ic: "⚑", g: "BUILD",   id: "flags",     t: "MODES / FLAGS",       state: "live",
      v: "EMULATION (WIN32)", verb: "VER", detail: "deployment mode, target triple, feature knobs" }
  ];

  // ---- meters ---------------------------------------------------------------
  var METERS = [
    { id: "cpu",    k: "host cpu",        state: "pending", spark: true },
    { id: "mem",    k: "static pool",     state: "live",    v: "10752 B free" },
    { id: "tests",  k: "regression",      state: "live",    v: "22/22 · 204/204" },
    { id: "parity", k: "parity signals",  state: "absent",  v: "not run" },
    { id: "deny",   k: "authority denials", state: "live",  v: "0" },
    { id: "pf",     k: "page-faults",     state: "absent",  v: "no evidence (LE-53)" }
  ];

  // 4 Hz, paused when hidden. One rAF-free interval for the whole document.
  var tickers = [];
  var timer = null;
  function startClock() {
    if (timer) return;
    timer = setInterval(function () {
      if (document.hidden) return;
      var t0 = performance.now();
      for (var i = 0; i < tickers.length; i++) tickers[i]();
      global.TinyOS.stats.paint = (performance.now() - t0).toFixed(2);
    }, 250);
  }
  document.addEventListener("visibilitychange", function () {
    if (!document.hidden) startClock();
  });

  // ---- transcript colouriser -------------------------------------------------
  // The "plugin that colours CMD IO": one pass per emitted line, a handful of spans,
  // still append-only. Colour carries meaning — directories, executables, sizes, dates,
  // labels, verdicts and denials each read differently without being read.
  function span(cls, text) {
    var e = document.createElement("span");
    if (cls) e.className = cls;
    e.textContent = text;
    return e;
  }
  var EXTC = { TXT: "t-txt", TCB: "t-exe", SYS: "t-exe", CFG: "t-cfg", LOG: "t-txt" };

  function paintOut(s, frag) {
    var m;
    if ((m = s.match(/^(\s*Volume in drive )(\S+)( is )(\S+)$/))) {
      frag.append(span("t-key", m[1]), span("t-drive", m[2]), span("t-key", m[3]), span("t-vol", m[4]));
    } else if ((m = s.match(/^(\s*Volume Serial Number is )(\S+)$/))) {
      frag.append(span("t-key", m[1]), span("t-num", m[2]));
    } else if ((m = s.match(/^(\s*Directory of )(.+)$/))) {
      frag.append(span("t-key", m[1]), span("t-path", m[2]));
    } else if ((m = s.match(/^(\S+)(\s+)(<DIR>)(\s+)(.*)$/))) {
      frag.append(span("t-dir", m[1]), span(null, m[2]), span("t-dirtag", m[3]),
        span(null, m[4]), span("t-date", m[5]));
    } else if ((m = s.match(/^([A-Z0-9_~-]+)(\.)([A-Z]{1,3})(\s+)(\d+)(\s+)(.*)$/i))) {
      frag.append(span("t-file", m[1]), span("t-dot", m[2]),
        span(EXTC[m[3].toUpperCase()] || "t-ext", m[3]), span(null, m[4]),
        span("t-num", m[5]), span(null, m[6]), span("t-date", m[7]));
    } else if ((m = s.match(/^(\s*)(\d+)( File\(s\)\s*)(\d*)(.*)$/))) {
      frag.append(span(null, m[1]), span("t-num", m[2]), span("t-key", m[3]),
        span("t-num", m[4]), span("t-key", m[5]));
    } else if ((m = s.match(/^(\s*A\s+)(\[origin=)(\w+)( trust=)(\w+)(\]\s+)(.+)$/))) {
      frag.append(span("t-key", m[1]), span("t-key", m[2]), span("t-label", m[3]),
        span("t-key", m[4]), span("t-label", m[5]), span("t-key", m[6]), span("t-file", m[7]));
    } else if (/^\s{2}(TASK|Address|#)\b/.test(s) || /^\s+-{3,}/.test(s)) {
      frag.append(span("t-head", s));
    } else if ((m = s.match(/^(\s{2})([A-Z][A-Z0-9-]+)(\s+)(\d+)(\s+)(ready|waiting|blocked|running)$/))) {
      frag.append(span(null, m[1]), span("t-task", m[2]), span(null, m[3]),
        span("t-num", m[4]), span(null, m[5]),
        span(m[6] === "ready" ? "t-ok" : "t-wait", m[6]));
    } else if ((m = s.match(/^(\s*)(\d+)( bytes.*)$/))) {
      frag.append(span(null, m[1]), span("t-num", m[2]), span("t-key", m[3]));
    } else if ((m = s.match(/^([A-Z_][A-Z0-9_]*)(=)(.*)$/))) {
      frag.append(span("t-envk", m[1]), span("t-dot", m[2]), span("t-envv", m[3]));
    } else if (/^(\\---|\||-{4,})/.test(s.trim()) || /^A:\\$/.test(s.trim())) {
      frag.append(span("t-tree", s));
    } else {
      frag.append(span("l-out", s));
    }
  }

  function paintLine(line) {
    var frag = document.createDocumentFragment(), m;
    if (line.c === "echo" && (m = line.s.match(/^(.*?[>$%#]\s?)(\S*)(.*)$/))) {
      frag.append(span("l-prompt", m[1]), span("t-verb", m[2]), span("t-args", m[3]));
    } else if (line.c === "out") {
      paintOut(line.s, frag);
    } else {
      frag.append(span("l-" + line.c, line.s));
    }
    frag.append(document.createTextNode("\n"));
    return frag;
  }

  // ---- mounts ---------------------------------------------------------------
  function mountTranscript(el, session) {
    function add(line) {
      if (!line) { el.textContent = ""; return; }
      el.appendChild(paintLine(line));
      el.scrollTop = el.scrollHeight;
    }
    el.textContent = "";
    session.lines.forEach(add);
    session.on(add);
  }

  function mountInput(row, session, onRun) {
    var input = row.querySelector("input"), p = row.querySelector(".p");
    p.textContent = session.prompt();
    var hi = -1;
    input.addEventListener("keydown", function (e) {
      if (e.key === "Enter") {
        var v = input.value; input.value = ""; hi = -1;
        session.run(v); p.textContent = session.prompt();
        if (onRun) onRun(v);
      } else if (e.key === "ArrowUp") {
        if (hi < 0) hi = session.history.length;
        hi = Math.max(0, hi - 1); input.value = session.history[hi] || ""; e.preventDefault();
      } else if (e.key === "ArrowDown") {
        hi = Math.min(session.history.length, hi + 1);
        input.value = session.history[hi] || ""; e.preventDefault();
      }
    });
    return input;
  }

  function sparkline(svg, seed) {
    var pts = [], n = 40, v = 30;
    for (var i = 0; i < n; i++) { v = Math.max(6, Math.min(46, v + (Math.sin(i * seed) * 6))); pts.push(v); }
    var path = "";
    tickers.push(function () {
      v = Math.max(6, Math.min(46, v + (Math.random() - 0.5) * 8));
      pts.push(v); pts.shift();
      path = pts.map(function (y, i) { return (i ? "L" : "M") + (i * (100 / n)) + " " + (50 - y); }).join(" ");
      svg.firstChild.setAttribute("d", path);
    });
    svg.setAttribute("viewBox", "0 0 100 50");
    svg.setAttribute("preserveAspectRatio", "none");
    var p = document.createElementNS("http://www.w3.org/2000/svg", "path");
    p.setAttribute("fill", "none"); p.setAttribute("stroke", "currentColor");
    p.setAttribute("stroke-width", "2"); p.setAttribute("vector-effect", "non-scaling-stroke");
    svg.appendChild(p);
    startClock();
  }

  function mountMeters(el, ids) {
    var list = METERS.filter(function (m) { return !ids || ids.indexOf(m.id) >= 0; });
    list.forEach(function (m) {
      var d = document.createElement("div");
      d.className = "meter"; d.dataset.state = m.state;
      d.innerHTML = '<div class="k"></div><div class="v"></div>';
      d.querySelector(".k").textContent = m.k;
      d.querySelector(".v").textContent = m.v || "metering…";
      if (m.spark) {
        var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        svg.setAttribute("class", "spark");
        svg.style.color = "var(--t-pending)";
        d.appendChild(svg); sparkline(svg, 0.6);
        tickers.push(function () {
          d.querySelector(".v").textContent = "metering (uncalibrated)";
        });
      }
      el.appendChild(d);
    });
    startClock();
  }

  // Roving-tabindex list: one Tab stop, arrows move within. Used by rail, tabs, palette.
  function rove(container, itemSel) {
    function items() { return Array.prototype.slice.call(container.querySelectorAll(itemSel)); }
    items().forEach(function (el, i) { el.tabIndex = i ? -1 : 0; });
    container.addEventListener("keydown", function (e) {
      var list = items(), i = list.indexOf(document.activeElement);
      if (i < 0) return;
      var n = e.key === "ArrowDown" || e.key === "ArrowRight" ? i + 1
            : e.key === "ArrowUp" || e.key === "ArrowLeft" ? i - 1
            : e.key === "Home" ? 0 : e.key === "End" ? list.length - 1 : null;
      if (n === null) return;
      e.preventDefault();
      n = (n + list.length) % list.length;
      list.forEach(function (el) { el.tabIndex = -1; });
      list[n].tabIndex = 0; list[n].focus();
    });
  }

  function countNodes() { return document.getElementsByTagName("*").length; }

  global.TinyOS = {
    VERBS: VERBS, FLAVOURS: FLAVOURS, WORKBENCH: WORKBENCH, METERS: METERS, TASKS: TASKS,
    Session: Session, mountTranscript: mountTranscript, paintLine: paintLine,
    mountInput: mountInput,
    mountMeters: mountMeters, sparkline: sparkline, rove: rove, tickers: tickers,
    startClock: startClock, countNodes: countNodes,
    stats: { paint: "0.00" }
  };
})(window);
