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
        var liveSeconds = 0;
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
                // Exit non-zero unless at least one frame decoded, so a run can
                // gate a Report rather than merely inform a reader.
                case "--strict":
                    strict = true;
                    break;
                default:
                    if (path is null && !args[i].StartsWith('-')) path = args[i];
                    break;
            }
        }

        List<Frame> frames;

        if (liveSeconds > 0)
        {
            var chosen = device ?? PickEthernet();
            if (chosen is null)
            {
                Console.Error.WriteLine("ti64dink: no capture device found; try --list");
                return 2;
            }
            Console.WriteLine($"ti64dink: listening {liveSeconds}s on {chosen}");
            var payloads = Live.Capture(chosen, liveSeconds, out var seen);
            Console.WriteLine($"ti64dink: {seen} TOS64 frame(s) captured");
            Console.WriteLine();
            // Each captured payload is decoded on its own: a TOS64 frame may be
            // a beacon or a transcript line rather than a spoor frame, and only
            // the ones carrying the magic produce records.
            frames = [];
            foreach (var payload in payloads) frames.AddRange(DecodeAll(payload));
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
        }
        if (frames.Count == 0)
        {
            Console.WriteLine("No SPOORJ01 frame found.");
            Console.WriteLine();
            Console.WriteLine("If the board is beaconing, the likely cause is the capture, not the board:");
            Console.WriteLine("  * pktmon etl2txt needs -v 3 to include frame bytes;");
            Console.WriteLine("  * the filter must be `pktmon filter add -d 0x88B5`.");
            return strict ? 1 : 0;
        }

        Report(frames);
        return 0;
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
        var rung = category == "Boot" || category == "Fault" ? Rung(target) : $"target={target}";
        return $"[{seq}] {category,-9} {actor,-7} {action,-9} {outcome,-8} {rung,-22} cost={cost}";
    }

    private static string? Name(string[] table, int index) =>
        index >= 0 && index < table.Length ? table[index] : null;

    // Mirrors kernel::spoor. Order is the discriminant order, so a value added
    // to the kernel and not here shows as REFUSED rather than as a wrong name.
    private static readonly string[] Categories =
        ["Scheduling", "Lock", "Wcet", "Dispatch", "Exec", "Memory", "Boot", "Fault", "Actuation", "Shell"];
    private static readonly string[] Actors = ["Kernel", "Exec", "Session"];
    private static readonly string[] Actions =
    [
        "Create", "Boost", "Restore", "Block", "Select", "Overrun", "ResetBudget",
        "Fault", "Terminate", "Restart", "Degrade", "Actuate", "Deadline", "VerbDenied"
    ];
    private static readonly string[] Outcomes =
        ["Ok", "Empty", "Chose", "Capped", "Failed", "Skipped", "Superseded", "Partial"];

    /// Mirrors kernel::spoor_stream::Rung. Wire-visible and append-only.
    private static string Rung(int target) => target switch
    {
        1 => "rung=MmuEnabled",
        2 => "rung=GicRouted",
        3 => "rung=TickArmed",
        4 => "rung=BeaconTransmitted",
        5 => "rung=FixtureMeasure",
        6 => "rung=ParkIteration",
        7 => "rung=FaultTaken",
        _ => $"rung=UNKNOWN({target})",
    };
}
