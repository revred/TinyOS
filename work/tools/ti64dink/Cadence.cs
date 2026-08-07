// Arrival time, and what may honestly be made from it (LE-115).
//
// The board's TOS64-PRESENT/1 beacon carries an incrementing seq at the park
// beat, which makes the beat cadence the cheapest live board evidence there is
// — no power cycle, no relay, no elevation. Until this file the tool printed
// frame text with no arrival time, so the best anchor a capture could give was
// the last seq seen before it ended: uncertain by a whole beat interval at
// each end, which put a 176 s baseline's period in [0.99653, 1.01938] s — a
// range that cannot support any verdict it would be taken for.
//
// The stamp is Stopwatch-based (monotonic, from capture start) because only
// DIFFERENCES matter; a wall clock adds nothing and can step backwards.
//
// What this file refuses is as load-bearing as what it computes:
//
//   * Per-frame jitter is NEVER offered. A host arrival time includes Windows
//     scheduling, Npcap buffering and NIC coalescing, so frame-to-frame
//     variation measured here is the host's, not the board's. What survives
//     averaging is the MEAN RATE over the span, because host delay averages
//     out while the board's counter does not. The summary says so itself, so
//     pasting it into a Report pastes its own boundary.
//   * A rate is never computed across a reboot. A backwards seq is a restart
//     (the tool's own rule 3: a reboot is never reported as loss), and a span
//     bridging two boots is a number about nothing.
//   * A seq that does not advance yields a refusal, not a division.

using System.Text;

internal static class Cadence
{
    /// The beacon line up to its sequence digits — one constant for the
    /// harvester in `WriteRawBeacons` and the sampler here, so the two cannot
    /// drift apart (the LE-80 shape, avoided rather than repaired).
    internal const string BeaconSeqPrefix = "TOS64-PRESENT/1 board=pi5-bcm2712 seq=";

    /// One beacon's arrival: seconds since capture start, and the sequence the
    /// board stamped into it.
    internal sealed record BeatSample(double Seconds, ulong Seq);

    /// Reads a payload as a beat sample, or nothing.
    ///
    /// The whole prefix is matched, board id included: a different board is a
    /// different counter, and splicing two clocks into one series would
    /// corrupt exactly the mean this file exists to make honest. A seq field
    /// with no digits is refused, never read as zero.
    internal static BeatSample? FromPayload(double seconds, byte[] payload)
    {
        if (payload.Length < BeaconSeqPrefix.Length)
        {
            return null;
        }
        var text = Encoding.ASCII.GetString(payload, 0,
            Math.Min(payload.Length, BeaconSeqPrefix.Length + 20));
        if (!text.StartsWith(BeaconSeqPrefix, StringComparison.Ordinal))
        {
            return null;
        }
        var rest = text[BeaconSeqPrefix.Length..];
        var end = 0;
        while (end < rest.Length && char.IsAsciiDigit(rest[end]))
        {
            end++;
        }
        return end == 0 ? null : new BeatSample(seconds, ulong.Parse(rest[..end]));
    }

    /// One line per live frame: the arrival stamp and what arrived.
    ///
    /// The stamp leads and is fixed-width so a capture log column-aligns; the
    /// description is the envelope line when the payload is one, the magic
    /// when it is a spoor frame, and an octet count otherwise — named, never
    /// guessed at.
    internal static string ArrivalLine(double seconds, byte[] payload)
    {
        string what;
        if (payload.Length >= 6 && Encoding.ASCII.GetString(payload, 0, 6) == "TOS64-")
        {
            var end = 0;
            while (end < payload.Length && payload[end] >= 0x20 && payload[end] <= 0x7E)
            {
                end++;
            }
            var line = Encoding.ASCII.GetString(payload, 0, end).TrimEnd();
            what = line.Length > 66 ? line[..66] : line;
        }
        else if (payload.Length >= 8 && Encoding.ASCII.GetString(payload, 0, 8) == "SPOORJ01")
        {
            what = "SPOORJ01 frame";
        }
        else
        {
            what = $"{payload.Length} octet payload";
        }
        return $"  +{seconds:F3}s  {what}";
    }

    /// The cadence summary, or the reason there is none.
    ///
    /// Returns the lines to print — empty when no beacon arrived, so the
    /// summary never claims a window said something it did not.
    internal static List<string> Summarise(IReadOnlyList<BeatSample> samples)
    {
        if (samples.Count == 0)
        {
            return [];
        }

        var lines = new List<string> { "", "---- beat cadence (host-timed, LE-115) ----" };
        if (samples.Count == 1)
        {
            lines.Add($"    one beacon (seq={samples[0].Seq} at +{samples[0].Seconds:F3}s) — "
                + "two are needed for a rate");
            return lines;
        }

        for (var i = 1; i < samples.Count; i++)
        {
            if (samples[i].Seq < samples[i - 1].Seq)
            {
                // The tool's rule 3, applied to arithmetic: a reboot is named,
                // and nothing is computed across it. A span bridging two boots
                // averages two unrelated counters.
                lines.Add($"    seq restarted ({samples[i - 1].Seq} -> {samples[i].Seq}): "
                    + "a reboot — rate not computed across boots");
                return lines;
            }
        }

        var first = samples[0];
        var last = samples[^1];
        var beats = last.Seq - first.Seq;
        if (beats == 0)
        {
            lines.Add($"    seq did not advance ({first.Seq} throughout) — no rate exists");
            return lines;
        }

        var span = last.Seconds - first.Seconds;
        lines.Add($"    first beacon : seq={first.Seq} at +{first.Seconds:F3}s");
        lines.Add($"    last beacon  : seq={last.Seq} at +{last.Seconds:F3}s");
        lines.Add($"    mean period  : {span / beats:F6} s  (dseq={beats}, span={span:F3}s)");
        lines.Add("    NOTE: host arrival times — the MEAN over the span is the evidence;");
        lines.Add("          per-frame jitter is the host's (Windows+Npcap+NIC), never the board's.");
        return lines;
    }
}
