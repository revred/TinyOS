// Ti64Dink — the host end of the TinyOS board link (FEAT-P2-10).
//
// Reads TOS64 spoor frames and decodes them against the kernel's own
// vocabularies. A spoor is to a physical system what a token is to a language
// model, so this is the program that reads the system's behaviour rather than
// a program that reads a log.
//
// Two rules it will not bend:
//
//   1. Loss is reported, never smoothed. Every gap in the frame sequence is
//      printed as an exact count of records that did not arrive. A viewer that
//      hides drops turns a partial stream into a confident lie.
//   2. Unknown discriminants are refused, not guessed. kernel::spoor::decode
//      fails closed on an unrecognised field and so does this; a guessed field
//      decodes to something plausible and meaningless, which is worse than a
//      hole because nothing marks it.
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
    private const int MaxRecords = 184;

    private static int Main(string[] args)
    {
        if (args.Length == 0 || args[0] is "-h" or "--help")
        {
            Usage();
            return args.Length == 0 ? 2 : 0;
        }

        string? path = null;
        var strict = false;
        for (var i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
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

        if (path is null) { Usage(); return 2; }
        if (!File.Exists(path))
        {
            Console.Error.WriteLine($"ti64dink: no such file: {path}");
            return 2;
        }

        var bytes = ReadCandidateBytes(path);
        Console.WriteLine($"ti64dink: {bytes.Length} candidate bytes from {Path.GetFileName(path)}");
        Console.WriteLine();

        var frames = DecodeAll(bytes);
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

    private sealed record Frame(ulong Seq, int Count, List<ulong> Records);

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

            var records = new List<ulong>(count);
            for (var i = 0; i < count; i++)
            {
                records.Add(BitConverter.ToUInt64(bytes, start + HeaderLen + i * 8));
            }
            frames.Add(new Frame(seq, count, records));
            at = start + HeaderLen + count * 8;
        }
        return frames;
    }

    private static void Report(List<Frame> frames)
    {
        ulong? expected = null;
        ulong lost = 0;
        var decoded = 0;
        var refused = 0;

        foreach (var frame in frames)
        {
            if (expected is { } want && frame.Seq != want)
            {
                if (frame.Seq > want)
                {
                    var gap = frame.Seq - want;
                    lost += gap;
                    // Printed, never smoothed. This line is the difference
                    // between a partial stream and a stream that lies.
                    Console.WriteLine($"  !! {gap} record(s) LOST before seq {frame.Seq}");
                }
                else
                {
                    Console.WriteLine($"  !! sequence went backwards: expected {want}, got {frame.Seq}");
                }
            }

            Console.WriteLine($"frame seq={frame.Seq} count={frame.Count}");
            for (var i = 0; i < frame.Records.Count; i++)
            {
                var line = Describe(frame.Records[i], frame.Seq + (ulong)i, ref decoded, ref refused);
                Console.WriteLine("    " + line);
            }
            expected = frame.Seq + (ulong)frame.Count;
        }

        Console.WriteLine();
        Console.WriteLine("---- summary ----");
        Console.WriteLine($"frames        : {frames.Count}");
        Console.WriteLine($"records ok    : {decoded}");
        Console.WriteLine($"records refused: {refused}   (unknown discriminant — refused, not guessed)");
        Console.WriteLine($"records lost  : {lost}");
        if (frames.Count > 0)
        {
            var first = frames[0].Seq;
            var last = frames[^1].Seq + (ulong)frames[^1].Count;
            Console.WriteLine($"sequence span : {first} .. {last}");
        }
        Console.WriteLine(lost == 0
            ? "stream        : continuous — no records lost across the captured window"
            : $"stream        : LOSSY — {lost} records dropped and accounted for");
    }

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
