// Ti64Dink — the host end of the TinyOS board link (FEAT-P2-10).
//
// Reads TOS64 spoor frames and decodes them against the kernel's own
// vocabularies. A spoor is to a physical system what a token is to a language
// model, so this is the program that reads the system's behaviour rather than
// a program that reads a log.
//
// Three rules it will not bend:
//
//   1. Loss is reported, never smoothed. Every gap in the frame sequence is
//      printed as an exact count of records that did not arrive. A viewer that
//      hides drops turns a partial stream into a confident lie.
//   2. Unknown discriminants are refused, not guessed. kernel::spoor::decode
//      fails closed on an unrecognised field and so does this; a guessed field
//      decodes to something plausible and meaningless, which is worse than a
//      hole because nothing marks it.
//   3. A reboot is never reported as loss. Since STORY-P1-10-04 every frame
//      carries the boot epoch that emitted it, so a stream that restarts is
//      named as a restart rather than counted as tens of thousands of records
//      dropped. The board can now tell a late listener which boot it joined;
//      this is the half that listens.
//
// It also harvests the OTHER thing on this cable. The board transmits its
// TOS64-MEAS/2 measurement envelope and its TOS64-PRESENT/1 beacon as TEXT
// frames on the same EtherType 0x88B5, one line per beat, cycling
// (hal_arm64::gem::text_frame). Until now this tool captured those frames and
// silently discarded them, because DecodeAll only ever looked for the SPOORJ01
// magic - so the measurement envelope has been riding the wire unread through
// every capture ever taken, including the three that produced BOARD VERDICTS
// 11-13. REPORT-2026-08-04-01's single largest named debt is that no
// board-emitted envelope has been machine-parsed off the wire. It was there the
// whole time; nothing was listening for it.
//
// Input today is a capture file (pktmon etl2txt text, or raw bytes). Live
// capture needs a driver below Windows' EtherType demux — Windows discards
// EtherType 0x88B5 before any socket can see it, and offers no user-mode API
// for raw Ethernet at any privilege level. Once Npcap is installed, live
// capture becomes a source that feeds these same decoders unchanged.

using System.Globalization;
using System.Text;

internal static class Program
{
    /// Frame magic — kernel::spoor_journal::JOURNAL_MAGIC. Identical to the
    /// journal's on-disk shape, so a capture and a journal file decode alike.
    private static readonly byte[] Magic = "SPOORJ01"u8.ToArray();

    private const int HeaderLen = 24;

    /// kernel::spoor_wire::MAX_RECORDS. 181 rather than 184 since the UDP
    /// framing joined the raw one: one constant keeps both inside an MTU.
    private const int MaxRecords = 181;

    /// kernel::spoor_wire::FLAG_RETAINED — this frame re-announces the boot
    /// certificate and carries sequence numbers already sent, so it must never
    /// be fed to the gap arithmetic.
    private const ushort FlagRetained = 0x0001;

    /// Every flag bit this decoder implements. A bit outside it is a newer
    /// board, and is named rather than ignored.
    private const ushort KnownFlags = FlagRetained;

    /// Every TOS64 envelope line begins with this. Used to find text frames
    /// among binary ones without needing to know which is which in advance:
    /// the board sends both on one EtherType so one capture filter sees the
    /// whole conversation, and this is the other half of that bargain.
    private const string EnvelopePrefix = "TOS64-";

    /// Shortest run of printable ASCII treated as an envelope line. Twelve is
    /// under the shortest real row ("END metrics=8") and well over the length a
    /// run of packed binary records produces by chance.
    private const int MinEnvelopeLine = 12;

    /// kernel::spoor_wire::EPOCH_UNDECLARED — an unseeded board, or an image
    /// older than the field. Read as an absence, never as boot number zero.
    private const uint EpochUndeclared = 0;

    private static int Main(string[] args)
    {
        if (args.Length == 0 || args[0] is "-h" or "--help")
        {
            Usage();
            return args.Length == 0 ? 2 : 0;
        }

        string? path = null;
        var strict = false;
        string? device = null;
        string? textPath = null;
        var anyFrames = false;
        var liveSeconds = 0;
        string? untilSpec = null;
        var timeoutSeconds = DefaultUntilTimeout;
        for (var i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--list":
                    foreach (var d in Live.Devices())
                    {
                        Console.WriteLine($"{d.Name}\n    {d.Description}");
                    }
                    return 0;
                case "--live":
                    liveSeconds = i + 1 < args.Length && int.TryParse(args[i + 1], out var s)
                        ? (i++, s).Item2
                        : 15;
                    break;
                case "--dev" when i + 1 < args.Length:
                    device = args[++i];
                    break;
                case "--file" when i + 1 < args.Length:
                    path = args[++i];
                    break;
                // Writes the harvested envelope lines to a file, so
                // `xtask parse-meas` can consume a WIRE capture rather than a
                // transcription of a photograph.
                case "--text" when i + 1 < args.Length:
                    textPath = args[++i];
                    break;
                // Watch the wire before TinyOS exists: the Pi 5 bootloader's
                // netboot traffic is DHCP and TFTP over IPv4/UDP, so the 0x88B5
                // filter that keeps every other capture clean is exactly what
                // hides this one.
                case "--any":
                    anyFrames = true;
                    break;
                // Exit non-zero unless at least one frame decoded, so a run can
                // gate a Report rather than merely inform a reader.
                case "--strict":
                    strict = true;
                    break;
                // Wait for a board event instead of guessing a window: the
                // capture ends the moment the condition is sighted, and the
                // exit code says which of "sighted" and "timed out" happened.
                case "--until" when i + 1 < args.Length:
                    untilSpec = args[++i];
                    break;
                case "--timeout" when i + 1 < args.Length && int.TryParse(args[i + 1], out var t):
                    timeoutSeconds = t;
                    i++;
                    break;
                default:
                    if (path is null && !args[i].StartsWith('-')) path = args[i];
                    break;
            }
        }

        List<Frame> frames;
        var text = new List<string>();

        Watch? watch = null;
        if (untilSpec is not null)
        {
            watch = Watch.Parse(untilSpec);
            if (watch is null)
            {
                Console.Error.WriteLine($"ti64dink: --until does not understand `{untilSpec}`");
                Console.Error.WriteLine("  conditions: epoch-change | rung=<Name> | text=<substring>");
                return 2;
            }
        }

        // A watch with no file goes live; a watch WITH a file evaluates the
        // file (the degenerate case a scripted test can drive with no board).
        if (anyFrames)
        {
            var anyDevice = device ?? PickEthernet();
            if (anyDevice is null)
            {
                Console.Error.WriteLine("ti64dink: no capture device found; try --list");
                return 2;
            }
            var window = liveSeconds > 0 ? liveSeconds : 60;
            Console.WriteLine($"ti64dink: --any, listening {window}s on {anyDevice}");
            Console.WriteLine("  (every EtherType, headers kept — this is the bootloader lane)");
            Console.WriteLine();
            var raw = Live.CaptureAny(anyDevice, window, out var rawSeen);
            ReportAny(raw, rawSeen);
            return raw.Count == 0 && strict ? 1 : 0;
        }

        if (liveSeconds > 0 || (watch is not null && path is null))
        {
            var chosen = device ?? PickEthernet();
            if (chosen is null)
            {
                Console.Error.WriteLine("ti64dink: no capture device found; try --list");
                return 2;
            }
            // `--until` owns the window when both are given: `--live N` states
            // a window, `--until` states a condition bounded by a deadline, and
            // the deadline is the larger discipline.
            var window = watch is not null ? timeoutSeconds : liveSeconds;
            Console.WriteLine(watch is not null
                ? $"ti64dink: watching for {watch.Describe} on {chosen} (timeout {window}s)"
                : $"ti64dink: listening {window}s on {chosen}");
            var startedAt = DateTime.UtcNow;
            var payloads = Live.Capture(chosen, window, watch is null ? null : watch.Offer, out var seen);
            Console.WriteLine($"ti64dink: {seen} TOS64 frame(s) captured");
            Console.WriteLine();
            // Each captured payload is decoded on its own: a TOS64 frame may be
            // a beacon or a transcript line rather than a spoor frame, and only
            // the ones carrying the magic produce records.
            frames = [];
            foreach (var payload in payloads)
            {
                frames.AddRange(DecodeAll(payload));
                HarvestText(payload, text);
            }
            if (watch is not null)
            {
                var waited = (DateTime.UtcNow - startedAt).TotalSeconds;
                Console.WriteLine(watch.Sighted
                    ? $"until: SIGHTED {watch.Describe} after {waited:F1}s"
                    : $"until: TIMEOUT — {watch.Describe} not seen within {window}s");
                Console.WriteLine();
                // The evidence below still prints either way; the exit code at
                // the bottom of Main is the machine-readable half.
            }
        }
        else
        {
            if (path is null) { Usage(); return 2; }
            if (!File.Exists(path))
            {
                Console.Error.WriteLine($"ti64dink: no such file: {path}");
                return 2;
            }

            var bytes = ReadCandidateBytes(path);
            Console.WriteLine($"ti64dink: {bytes.Length} candidate bytes from {Path.GetFileName(path)}");
            Console.WriteLine();
            frames = DecodeAll(bytes);
            HarvestText(bytes, text);
            if (watch is not null)
            {
                // The degenerate `--until` over a file: no waiting, but the
                // same condition logic and the same exit code, so a scripted
                // test can exercise the watch with no board on the bench.
                watch.Offer(bytes);
                Console.WriteLine(watch.Sighted
                    ? $"until: SIGHTED {watch.Describe} in this capture"
                    : $"until: {watch.Describe} is not in this capture");
                Console.WriteLine();
            }
        }
        if (frames.Count == 0)
        {
            // Text first: a capture holding the measurement envelope and no
            // spoor frames is a SUCCESSFUL capture of a different thing, and
            // reporting it as "nothing found" would throw away the evidence
            // this tool was just taught to see.
            ReportText(text, textPath);
            Console.WriteLine("No SPOORJ01 frame found.");
            Console.WriteLine();
            Console.WriteLine("If the board is beaconing, the likely cause is the capture, not the board:");
            Console.WriteLine("  * pktmon etl2txt needs -v 3 to include frame bytes;");
            Console.WriteLine("  * the filter must be `pktmon filter add -d 0x88B5`.");
            return watch is { Sighted: false } ? 1 : (strict ? 1 : 0);
        }

        Report(frames);
        ReportText(text, textPath);
        return watch is { Sighted: false } ? 1 : 0;
    }

    /// `--until`'s default deadline. Two minutes: longer than a boot plus a
    /// full announcement period by an order of magnitude, short enough that a
    /// wrong condition fails a script rather than parking a bench overnight.
    /// A UX bound, not a measured one — override with `--timeout`.
    private const int DefaultUntilTimeout = 120;

    /// One `--until` condition: what to look for, whether it has been seen.
    ///
    /// The three primitives are the ones `hand-2026-08-05/01A` §4 asked for by
    /// name — an epoch change, a rung appearing, a value crossing a bound (the
    /// last via `text=`, since every envelope value rides a text line). An
    /// unknown condition or rung name is refused at parse, not guessed at.
    private sealed class Watch
    {
        private enum Kind { EpochChange, Rung, Text }

        private readonly Kind _kind;
        private readonly int _target;
        private readonly string _needle = "";
        private uint? _firstEpoch;
        private readonly List<string> _scratch = [];

        internal bool Sighted { get; private set; }
        internal string Describe { get; private init; } = "";

        private Watch(Kind kind, int target, string needle, string describe)
        {
            _kind = kind;
            _target = target;
            _needle = needle;
            Describe = describe;
        }

        internal static Watch? Parse(string spec)
        {
            if (spec == "epoch-change")
            {
                return new Watch(Kind.EpochChange, 0, "", "epoch-change (a reboot)");
            }
            if (spec.StartsWith("rung=", StringComparison.Ordinal))
            {
                var name = spec["rung=".Length..];
                foreach (var (target, known) in Rungs)
                {
                    if (known == name)
                    {
                        return new Watch(Kind.Rung, target, "", $"rung={name}");
                    }
                }
                return null; // an unknown rung is refused, not guessed
            }
            if (spec.StartsWith("text=", StringComparison.Ordinal) && spec.Length > "text=".Length)
            {
                var needle = spec["text=".Length..];
                return new Watch(Kind.Text, 0, needle, $"text containing `{needle}`");
            }
            return null;
        }

        /// Offers one payload (or a whole file's bytes); returns Sighted so
        /// the live loop can stop at the moment the condition is met.
        internal bool Offer(byte[] payload)
        {
            if (Sighted) return true;
            switch (_kind)
            {
                case Kind.EpochChange:
                    foreach (var frame in DecodeAll(payload))
                    {
                        // An undeclared epoch is an absence, not boot zero —
                        // it neither sets the baseline nor fires the change.
                        if (frame.Epoch == EpochUndeclared) continue;
                        if (_firstEpoch is null) _firstEpoch = frame.Epoch;
                        else if (frame.Epoch != _firstEpoch) Sighted = true;
                    }
                    break;
                case Kind.Rung:
                    foreach (var frame in DecodeAll(payload))
                    {
                        // Retained frames replay records already sent; a watch
                        // that fired on a re-announcement would report an old
                        // event as a fresh one.
                        if (frame.Retained) continue;
                        foreach (var bits in frame.Records)
                        {
                            var cat = (int)((bits >> 60) & 0xF);
                            var carriesRung = cat is 6 or 7 or 10; // Boot, Fault, Thermal
                            if (carriesRung && (int)((bits >> 32) & 0xFFFF) == _target)
                            {
                                Sighted = true;
                            }
                        }
                    }
                    break;
                case Kind.Text:
                    _scratch.Clear();
                    HarvestText(payload, _scratch);
                    foreach (var line in _scratch)
                    {
                        if (line.Contains(_needle, StringComparison.Ordinal)) Sighted = true;
                    }
                    break;
            }
            return Sighted;
        }
    }

    /// Harvests every TOS64 envelope line from a payload or a capture blob.
    ///
    /// Deliberately a scan rather than a frame parse, for the same reason
    /// ReadCandidateBytes is format-tolerant: this has to work against a live
    /// payload, a raw dump and one capture tool's text export without three
    /// code paths. A line is a run of printable ASCII starting at the envelope
    /// prefix and ending at the first NUL, CR or LF - which is exactly the
    /// shape hal_arm64::gem::text_frame emits, since it zero-pads to the
    /// Ethernet minimum.
    ///
    /// Duplicates are kept out: the board cycles its transcript one line per
    /// beat, so a 60-second capture holds each line many times over, and a
    /// reader wants the envelope, not the repetition count.
    private static void HarvestText(byte[] bytes, List<string> into)
    {
        var at = 0;
        while (at < bytes.Length)
        {
            // Maximal run of printable ASCII. `gem::text_frame` writes the line
            // at payload offset 0 and zero-pads to the Ethernet minimum, so a
            // run terminated by NUL is exactly one envelope line.
            if (bytes[at] < 0x20 || bytes[at] > 0x7E) { at++; continue; }
            var start = at;
            while (at < bytes.Length && bytes[at] >= 0x20 && bytes[at] <= 0x7E) at++;
            var line = Encoding.ASCII.GetString(bytes, start, at - start).TrimEnd();

            // Anchoring on the "TOS64-" prefix was the obvious rule and it is
            // WRONG: only the BEGIN line of a TOS64-MEAS/2 envelope carries it.
            // The metric rows ("D04  context_switch... min=80"), the continuation
            // row and "END metrics=8" do not, and those are the measurements -
            // the entire reason to read text frames at all.
            //
            // So the rule is shape, not prefix: long enough not to be noise, and
            // carrying a key=value pair, which every envelope row does and a run
            // of packed spoor records essentially never does. A false positive
            // prints as visible junk rather than corrupting a parse.
            if (line.Length >= MinEnvelopeLine && line.Contains('=') && !into.Contains(line))
            {
                into.Add(line);
            }
        }
    }

    /// Prints the harvested envelope, and optionally writes it where
    /// `xtask parse-meas` can read it.
    private static void ReportText(List<string> text, string? textPath)
    {
        if (text.Count == 0) return;

        Console.WriteLine();
        Console.WriteLine($"---- TOS64 text frames ---- ({text.Count} distinct line(s))");
        foreach (var line in text) Console.WriteLine("    " + line);

        if (textPath is null)
        {
            Console.WriteLine();
            Console.WriteLine("    (--text <file> writes these where `xtask parse-meas` can read them)");
            return;
        }

        try
        {
            File.WriteAllLines(textPath, EnvelopeForParser(text));
            Console.WriteLine();
            Console.WriteLine($"    written to {textPath} — parse with:");
            Console.WriteLine($"      cargo run -p xtask -- parse-meas --file={textPath}");
        }
        catch (IOException e)
        {
            // Reported, never swallowed: a capture that silently failed to save
            // is a capture that has to be taken again on a board someone has
            // already powered down.
            Console.Error.WriteLine($"ti64dink: could not write {textPath}: {e.Message}");
        }
    }

    /// The wired adapter, chosen by description rather than by order.
    ///
    /// Loopback and virtual adapters are excluded by name; if more than one
    /// candidate remains the first is taken and printed, so a wrong guess is
    /// visible rather than silent.
    private static string? PickEthernet()
    {
        var devices = Live.Devices();
        foreach (var d in devices)
        {
            var text = (d.Description + " " + d.Name).ToLowerInvariant();
            // Excluded first and by what they are, not by vendor: an adapter
            // named "Intel(R) Wi-Fi 6 AX201" matches a naive vendor test and is
            // emphatically not the cable the board is on.
            if (text.Contains("loopback") || text.Contains("virtual")
                || text.Contains("wan miniport") || text.Contains("wi-fi")
                || text.Contains("wifi") || text.Contains("wireless")
                || text.Contains("bluetooth"))
            {
                continue;
            }
            if (text.Contains("ethernet") || text.Contains("gbe"))
            {
                return d.Name;
            }
        }
        return devices.Count > 0 ? devices[0].Name : null;
    }

    private static void Usage()
    {
        Console.WriteLine("Ti64Dink — decode TinyOS spoor frames (FEAT-P1-10 / FEAT-P2-10)");
        Console.WriteLine();
        Console.WriteLine("  ti64dink --file <capture>   pktmon etl2txt output, or raw frame bytes");
        Console.WriteLine("  ti64dink --file <c> --strict  exit 1 unless at least one frame decodes");
        Console.WriteLine("  ti64dink --live 30 --text env.txt   also harvest the TOS64-MEAS/2");
        Console.WriteLine("                                      envelope the board transmits as text");
        Console.WriteLine("                                      frames, for `xtask parse-meas`");
        Console.WriteLine();
        Console.WriteLine("  ti64dink --until <cond> [--timeout 120]   wait for a board event instead");
        Console.WriteLine("                                            of guessing a window; exits 0 the");
        Console.WriteLine("                                            moment it is sighted, 1 on timeout");
        Console.WriteLine("      epoch-change        a reboot (the boot epoch changed)");
        Console.WriteLine("      rung=<Name>         a fresh record with that rung (e.g. rung=ThermalSample)");
        Console.WriteLine("      text=<substring>    a TOS64 text line containing <substring>");
        Console.WriteLine();
        Console.WriteLine("Capture on Windows (elevated, until Npcap is installed):");
        Console.WriteLine("  pktmon filter remove");
        Console.WriteLine("  pktmon filter add -d 0x88B5");
        Console.WriteLine("  pktmon start --capture --pkt-size 0 --file-name $env:TEMP\\spoor.etl");
        Console.WriteLine("  pktmon stop");
        Console.WriteLine("  pktmon etl2txt $env:TEMP\\spoor.etl -o $env:TEMP\\spoor.txt -v 3");
    }

    /// Returns every byte the file plausibly contains.
    ///
    /// Deliberately format-tolerant: rather than parse one capture tool's
    /// layout, it recovers a byte stream and lets the magic find itself. A
    /// decoder that only works against one version of one tool's text output is
    /// a decoder that stops working on a bench where it matters.
    private static byte[] ReadCandidateBytes(string path)
    {
        var raw = File.ReadAllBytes(path);

        // If the magic is already present as bytes, it is a binary capture.
        if (IndexOf(raw, Magic, 0) >= 0) return raw;

        // Otherwise treat it as text and harvest hex byte tokens.
        var text = Encoding.UTF8.GetString(raw);
        var buffer = new List<byte>(raw.Length / 2);
        foreach (var token in text.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries))
        {
            var t = token.Trim();
            // pktmon -v 3 emits two-hex-digit byte columns; offsets and other
            // decimal noise are skipped by requiring exactly two hex digits.
            if (t.Length == 2 && IsHex(t[0]) && IsHex(t[1]))
            {
                buffer.Add(byte.Parse(t, NumberStyles.HexNumber, CultureInfo.InvariantCulture));
            }
        }
        return buffer.Count > 0 ? buffer.ToArray() : raw;
    }

    private static bool IsHex(char c) =>
        c is >= '0' and <= '9' or >= 'a' and <= 'f' or >= 'A' and <= 'F';

    private static int IndexOf(byte[] haystack, byte[] needle, int from)
    {
        for (var i = from; i + needle.Length <= haystack.Length; i++)
        {
            var hit = true;
            for (var j = 0; j < needle.Length; j++)
            {
                if (haystack[i + j] != needle[j]) { hit = false; break; }
            }
            if (hit) return i;
        }
        return -1;
    }

    private sealed record Frame(ulong Seq, int Count, uint Epoch, ushort Flags, List<ulong> Records)
    {
        /// A re-announcement of the boot certificate rather than fresh stream.
        internal bool Retained => (Flags & FlagRetained) != 0;

        /// The sequence to expect next, or null when this frame says nothing
        /// about that. Mirrors kernel::spoor_wire::FrameHeader::expected_next,
        /// and for the same reason: a retained frame carries sequence numbers
        /// that were already sent, so the arithmetic that would invent a gap
        /// is made unreachable rather than merely discouraged.
        internal ulong? ExpectedNext => Retained ? null : Seq + (ulong)Count;
    }

    private static List<Frame> DecodeAll(byte[] bytes)
    {
        var frames = new List<Frame>();
        var at = 0;
        while (true)
        {
            var start = IndexOf(bytes, Magic, at);
            if (start < 0) break;
            at = start + Magic.Length;

            if (start + HeaderLen > bytes.Length) break;
            var seq = BitConverter.ToUInt64(bytes, start + 8);
            int count = BitConverter.ToUInt16(bytes, start + 16);

            // The one field an attacker or a corrupt capture could inflate,
            // bounded against the format's own constant BEFORE it is used to
            // size a read. Same posture the board-side decoder takes.
            if (count > MaxRecords) continue;
            if (start + HeaderLen + count * 8 > bytes.Length) continue;

            // Neither of these is validated and neither can be: any 32-bit
            // value is a legal epoch, and an unknown flag bit means a newer
            // board rather than a malformed frame.
            var flags = BitConverter.ToUInt16(bytes, start + 18);
            var epoch = BitConverter.ToUInt32(bytes, start + 20);

            var records = new List<ulong>(count);
            for (var i = 0; i < count; i++)
            {
                records.Add(BitConverter.ToUInt64(bytes, start + HeaderLen + i * 8));
            }
            frames.Add(new Frame(seq, count, epoch, flags, records));
            at = start + HeaderLen + count * 8;
        }
        return frames;
    }

    /// What one boot looked like in this capture.
    ///
    /// Per boot rather than per capture, because a window that spans a reboot
    /// holds two unrelated sequence spaces and a single running total across
    /// both is a number about nothing.
    private sealed class Boot(uint epoch)
    {
        internal uint Epoch { get; } = epoch;
        internal ulong? FirstSeq;
        internal ulong? NextSeq;
        internal ulong Lost;
        /// Records of the seq-0 stream frame, if this capture caught the boot live.
        internal List<ulong>? LiveZero;
        /// Records of the most recent retained certificate for this boot.
        internal List<ulong>? Certificate;
    }

    private static void Report(List<Frame> frames)
    {
        ulong? expected = null;
        uint? epoch = null;
        ulong lost = 0;
        var decoded = 0;
        var refused = 0;
        var retainedFrames = 0;
        var boots = new List<Boot>();
        Boot? boot = null;

        foreach (var frame in frames)
        {
            // The boot epoch is read BEFORE any sequence arithmetic, because a
            // reboot is the one thing that makes that arithmetic meaningless: a
            // fresh boot restarts at seq 0, and a decoder that did not look
            // here first would report the restart as a backwards jump or the
            // join as tens of thousands of lost records. Neither happened.
            if (epoch is { } was && frame.Epoch != was)
            {
                Console.WriteLine();
                Console.WriteLine($"== BOOT CHANGED: epoch {Epoch(was)} -> {Epoch(frame.Epoch)} ==");
                Console.WriteLine("   a reboot, not loss — the sequence restart below is expected");
                expected = null;
            }
            if (epoch != frame.Epoch || boot is null)
            {
                epoch = frame.Epoch;
                boot = boots.Find(b => b.Epoch == frame.Epoch);
                if (boot is null)
                {
                    boot = new Boot(frame.Epoch);
                    boots.Add(boot);
                }
            }

            var unknownFlags = (ushort)(frame.Flags & ~KnownFlags);
            if (unknownFlags != 0)
            {
                // Named, not ignored: a bit this decoder does not implement is
                // a board newer than this build, and silently dropping it is
                // how a host quietly stops understanding what it is reading.
                Console.WriteLine($"  ?? frame carries flag bits 0x{unknownFlags:X4} this build does not know");
            }

            if (frame.Retained)
            {
                // A verbatim re-send of records already accounted for. It gets
                // no gap arithmetic and does not advance the expectation —
                // ExpectedNext returns null and this loop honours that.
                retainedFrames++;
                boot.Certificate = frame.Records;
                Console.WriteLine();
                Console.WriteLine($"== boot certificate (epoch {Epoch(frame.Epoch)}) — {frame.Count} record(s), re-announced ==");
                var ignoredDecoded = 0;
                var ignoredRefused = 0;
                for (var i = 0; i < frame.Records.Count; i++)
                {
                    Console.WriteLine("    " + Describe(
                        frame.Records[i], frame.Seq + (ulong)i, ref ignoredDecoded, ref ignoredRefused));
                }
                Console.WriteLine();
                continue;
            }

            if (expected is { } want && frame.Seq != want)
            {
                if (frame.Seq > want)
                {
                    var gap = frame.Seq - want;
                    lost += gap;
                    boot.Lost += gap;
                    // Printed, never smoothed. This line is the difference
                    // between a partial stream and a stream that lies.
                    Console.WriteLine($"  !! {gap} record(s) LOST before seq {frame.Seq}");
                }
                else
                {
                    Console.WriteLine($"  !! sequence went backwards: expected {want}, got {frame.Seq}");
                }
            }

            if (frame.Seq == 0) boot.LiveZero = frame.Records;
            boot.FirstSeq ??= frame.Seq;
            boot.NextSeq = frame.ExpectedNext;
            Console.WriteLine($"frame seq={frame.Seq} count={frame.Count} epoch={Epoch(frame.Epoch)}");
            for (var i = 0; i < frame.Records.Count; i++)
            {
                var line = Describe(frame.Records[i], frame.Seq + (ulong)i, ref decoded, ref refused);
                Console.WriteLine("    " + line);
            }
            expected = frame.ExpectedNext;
        }

        var streamFrames = frames.Count - retainedFrames;

        Console.WriteLine();
        Console.WriteLine("---- summary ----");
        Console.WriteLine($"frames        : {frames.Count}   ({streamFrames} stream, {retainedFrames} retained)");
        Console.WriteLine($"records ok    : {decoded}");
        Console.WriteLine($"records refused: {refused}   (unknown discriminant — refused, not guessed)");
        Console.WriteLine($"records lost  : {lost}");
        Console.WriteLine($"boots seen    : {boots.Count}");
        Console.WriteLine(lost == 0
            ? "stream        : continuous — no records lost across the captured window"
            : $"stream        : LOSSY — {lost} records dropped and accounted for");

        // Per boot, because two boots are two unrelated sequence spaces and a
        // span across both would be a number about nothing.
        foreach (var b in boots) ReportBoot(b);
    }

    /// What this capture can and cannot say about one boot.
    private static void ReportBoot(Boot boot)
    {
        Console.WriteLine();
        Console.WriteLine($"---- boot {Epoch(boot.Epoch)} ----");
        Console.WriteLine(boot.FirstSeq is { } first
            ? $"sequence span : {first} .. {boot.NextSeq?.ToString() ?? "?"}   ({boot.Lost} lost)"
            : "sequence span : none — only a retained certificate was seen for this boot");

        if (boot.Certificate is { } certificate)
        {
            if (boot.LiveZero is { } live)
            {
                // Both halves are present, so the verbatim claim is checkable
                // rather than assertable: STORY-P1-10-04 says the announcement
                // is the same bytes, and this is the capture that proves or
                // disproves it. A disagreement is a real finding and is
                // reported as one.
                var agrees = live.Count >= certificate.Count;
                for (var i = 0; agrees && i < certificate.Count; i++)
                {
                    agrees = live[i] == certificate[i];
                }
                Console.WriteLine(agrees
                    ? $"boot state    : captured live AND re-announced — {certificate.Count} record(s) byte-identical"
                    : "boot state    : !! the certificate DISAGREES with the live frame 0 — announcement is not verbatim");
            }
            else
            {
                Console.WriteLine("boot state    : recovered from a retained certificate — frame 0 was never captured");
            }
            var d = 0;
            var r = 0;
            for (var i = 0; i < certificate.Count; i++)
            {
                Console.WriteLine("    " + Describe(certificate[i], (ulong)i, ref d, ref r));
            }
        }
        else if (boot.LiveZero is not null)
        {
            Console.WriteLine("boot state    : captured live from frame 0; no announcement in this window");
        }
        else
        {
            Console.WriteLine("boot state    : UNKNOWN — no frame 0 and no certificate in this window.");
            Console.WriteLine("                Listen at least one announcement period, or the board");
            Console.WriteLine("                predates STORY-P1-10-04 and cannot re-announce at all.");
        }
    }

    /// An epoch as a reader should think of it: a boot's identity, or an honest
    /// absence. Never rendered as a number that could be mistaken for a count —
    /// it distinguishes boots and cannot number them (LE-74).
    private static string Epoch(uint epoch) =>
        epoch == EpochUndeclared ? "undeclared" : $"0x{epoch:X8}";

    private static string Describe(ulong bits, ulong seq, ref int decoded, ref int refused)
    {
        var cat = (int)((bits >> 60) & 0xF);
        var who = (int)((bits >> 56) & 0xF);
        var act = (int)((bits >> 52) & 0xF);
        var outc = (int)((bits >> 48) & 0xF);
        var target = (int)((bits >> 32) & 0xFFFF);
        var cost = (uint)(bits & 0xFFFF_FFFF);

        var category = Name(Categories, cat);
        var actor = Name(Actors, who);
        var action = Name(Actions, act);
        var outcome = Name(Outcomes, outc);

        if (category is null || actor is null || action is null || outcome is null)
        {
            refused++;
            return $"[{seq}] REFUSED raw=0x{bits:X16} " +
                   $"(cat={cat} who={who} act={act} out={outc} — unknown discriminant)";
        }

        decoded++;
        var rung = category is "Boot" or "Fault" or "Thermal" ? Rung(target) : $"target={target}";
        // The cost field is a raw AVS register word for a thermal sample, not a
        // number of cycles. Conversion is the HOST's job by design: the board
        // emits and does not interpret, and the calibration below is unverified
        // on this hardware, so it is labelled as a reading of the register
        // rather than presented as a measured temperature.
        var detail = category == "Thermal" ? Thermal(cost) : $"cost={cost}";
        return $"[{seq}] {category,-9} {actor,-7} {action,-9} {outcome,-8} {rung,-22} {detail}";
    }

    private static string? Name(string[] table, int index) =>
        index >= 0 && index < table.Length ? table[index] : null;

    // Mirrors kernel::spoor. Order is the discriminant order, so a value added
    // to the kernel and not here shows as REFUSED rather than as a wrong name.
    private static readonly string[] Categories =
    [
        "Scheduling", "Lock", "Wcet", "Dispatch", "Exec", "Memory", "Boot", "Fault",
        "Actuation", "Shell", "Thermal"
    ];
    private static readonly string[] Actors = ["Kernel", "Exec", "Session"];
    private static readonly string[] Actions =
    [
        "Create", "Boost", "Restore", "Block", "Select", "Overrun", "ResetBudget",
        "Fault", "Terminate", "Restart", "Degrade", "Actuate", "Deadline", "VerbDenied",
        "Observe"
    ];
    private static readonly string[] Outcomes =
        ["Ok", "Empty", "Chose", "Capped", "Failed", "Skipped", "Superseded", "Partial"];

    /// Summarises an unfiltered capture: who spoke, what EtherType, and any
    /// readable text in the payload.
    ///
    /// Deliberately a summary and not a protocol decoder. Writing a DHCP and
    /// TFTP parser to answer "what does the bootloader ask for" would be
    /// building the thing the answer is supposed to size — the same
    /// design-before-ground-truth mistake the netboot investigation exists to
    /// avoid. Ethernet addresses, EtherType, length and printable strings are
    /// enough to read a DHCP DISCOVER's vendor class and a TFTP read request's
    /// filename, which is the whole question.
    private static void ReportAny(List<byte[]> frames, int seen)
    {
        Console.WriteLine($"ti64dink: {seen} frame(s) captured");
        Console.WriteLine();

        var byType = new Dictionary<ushort, int>();
        var talkers = new Dictionary<string, int>();
        var strings = new List<string>();

        foreach (var f in frames)
        {
            var etherType = (ushort)((f[12] << 8) | f[13]);
            byType[etherType] = byType.GetValueOrDefault(etherType) + 1;
            var src = string.Join(':', f[6..12].Select(b => b.ToString("x2")));
            talkers[src] = talkers.GetValueOrDefault(src) + 1;

            // Printable runs of 4+, relaxed from the envelope rule: a TFTP
            // filename carries no `=` and is exactly what we are here to read.
            var at = 14;
            while (at < f.Length)
            {
                if (f[at] < 0x20 || f[at] > 0x7E) { at++; continue; }
                var start = at;
                while (at < f.Length && f[at] >= 0x20 && f[at] <= 0x7E) at++;
                if (at - start < 4) continue;
                var text = Encoding.ASCII.GetString(f, start, at - start).Trim();
                if (text.Length >= 4 && !strings.Contains(text)) strings.Add(text);
            }
        }

        Console.WriteLine("---- EtherTypes ----");
        foreach (var (type, count) in byType.OrderByDescending(e => e.Value))
        {
            var name = type switch
            {
                0x0800 => "IPv4 (DHCP/TFTP live here)",
                0x0806 => "ARP",
                0x86DD => "IPv6",
                0x88B5 => "TOS64 (the board's own)",
                _ => "",
            };
            Console.WriteLine($"    0x{type:X4}  {count,5} frame(s)  {name}");
        }

        Console.WriteLine();
        Console.WriteLine("---- source MACs ----");
        foreach (var (mac, count) in talkers.OrderByDescending(e => e.Value))
        {
            Console.WriteLine($"    {mac}  {count,5} frame(s)");
        }

        Console.WriteLine();
        Console.WriteLine($"---- readable strings ---- ({strings.Count} distinct)");
        foreach (var text in strings) Console.WriteLine("    " + text);
    }

    /// The TOS64-MEAS/2 envelope alone, in the order the board emits it.
    ///
    /// Two transformations, both of which have to be defensible because this
    /// file becomes evidence that `xtask parse-meas` reads.
    ///
    /// FILTERED to `TOS64-MEAS/2` lines. The board cycles one transcript line
    /// per beat and interleaves its `TOS64-PRESENT/1` beacon between them, so a
    /// raw harvest is two streams braided together. The beacon is still printed
    /// to the console - it is FEAT-P1-09's own evidence - but it is not part of
    /// a measurement envelope and the parser is right to refuse it.
    ///
    /// ROTATED so BEGIN comes first. A capture starts wherever the cycle
    /// happened to be, so a harvest is a ROTATION of the emission order, not a
    /// shuffle of it. Rotating back is exact and preserves every relative
    /// position; it is not a sort, and nothing is reordered within the run.
    /// If no BEGIN was captured the lines are written untouched, so the parser
    /// refuses an incomplete envelope rather than being handed a plausible one.
    private static List<string> EnvelopeForParser(List<string> text)
    {
        var envelope = text.FindAll(line => line.StartsWith("TOS64-MEAS/2", StringComparison.Ordinal));
        var begin = envelope.FindIndex(line => line.Contains(" BEGIN ", StringComparison.Ordinal));
        if (begin <= 0) return envelope;
        var rotated = envelope.GetRange(begin, envelope.Count - begin);
        rotated.AddRange(envelope.GetRange(0, begin));
        return rotated;
    }

    /// Renders an AVS monitor temperature word (LE-75).
    ///
    /// The raw register is printed FIRST and always, because it is the only
    /// part of this line that is measured. The celsius figure is derived with
    /// the bcm2711_thermal slope/offset, which has NOT been verified against
    /// this board - so it is marked and must not be quoted as evidence until a
    /// paired capture against thermal_zone0 confirms it.
    ///
    /// If the register offset is wrong, this is where it shows: a word that
    /// does not drift the way a die temperature drifts, or validity bits that
    /// never set, is visible here rather than hidden behind a plausible number.
    private static string Thermal(uint raw)
    {
        var data = raw & 0x3FF;
        var valid = (raw & (1u << 16)) != 0 || (raw & (1u << 10)) != 0;
        // bcm2711_thermal: millicelsius = -487 * data + 410040. UNVERIFIED here.
        var milli = -487.0 * data + 410040.0;
        var flag = valid ? "" : " INVALID";
        return $"avs=0x{raw:X8} data={data}{flag} ~{milli / 1000.0:F1}C(unverified)";
    }

    /// Mirrors kernel::spoor_stream::Rung. Wire-visible and append-only. One
    /// table for both directions: the decoder renders from it and `--until
    /// rung=<Name>` resolves against it, so the two cannot drift apart.
    private static readonly (int Target, string Name)[] Rungs =
    [
        (1, "MmuEnabled"),
        (2, "GicRouted"),
        (3, "TickArmed"),
        (4, "BeaconTransmitted"),
        (5, "FixtureMeasure"),
        (6, "ParkIteration"),
        (7, "FaultTaken"),
        (8, "ThermalSample"),
    ];

    private static string Rung(int target)
    {
        foreach (var (known, name) in Rungs)
        {
            if (known == target) return $"rung={name}";
        }
        return $"rung=UNKNOWN({target})";
    }
}
