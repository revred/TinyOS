// LE-115 — the instrument discarded the one field the question needed.
//
// TOS64-PRESENT/1 carries an incrementing seq at the 1 Hz park beat, which
// makes the beat cadence the cheapest board evidence available — but ti64dink
// printed frame text with no arrival time, so the best anchor was the last seq
// before a capture ended, uncertain by a whole beat interval at each end. The
// measured consequence: a 176.392 s baseline resolved the period only to
// [0.99653, 1.01938] s, unable to support any verdict it would be taken for.
//
// These tests pin the fix's two halves: a per-frame arrival stamp (Stopwatch
// seconds from capture start — only differences matter), and a cadence summary
// that reports the MEAN rate over the span and refuses everything else. The
// refusals matter as much as the number: per-frame jitter measured this way is
// the host's (Windows scheduling, Npcap buffering, NIC coalescing), never the
// board's, and a rate computed across a reboot is a number about nothing.

using System.Text;

public sealed class CadenceTests
{
    private static byte[] Beacon(ulong seq) =>
        Encoding.ASCII.GetBytes($"TOS64-PRESENT/1 board=pi5-bcm2712 seq={seq} up=1");

    [Fact]
    public void a_beacon_payload_yields_its_sequence_and_arrival()
    {
        var sample = Cadence.FromPayload(2.125, Beacon(175));
        Assert.NotNull(sample);
        Assert.Equal(175UL, sample!.Seq);
        Assert.Equal(2.125, sample.Seconds);
    }

    [Fact]
    public void a_non_beacon_line_is_not_a_beat_sample()
    {
        Assert.Null(Cadence.FromPayload(1.0,
            Encoding.ASCII.GetBytes("TOS64-MEAS/2 BEGIN fixture=measure")));
        // The prefix is matched whole: a different board id is a different
        // stream, and counting it into this one would splice two clocks.
        Assert.Null(Cadence.FromPayload(1.0,
            Encoding.ASCII.GetBytes("TOS64-PRESENT/1 board=other seq=3")));
        // A beacon whose seq field carries no digits is refused, not read as 0.
        Assert.Null(Cadence.FromPayload(1.0,
            Encoding.ASCII.GetBytes("TOS64-PRESENT/1 board=pi5-bcm2712 seq=")));
    }

    [Fact]
    public void no_beacons_yields_no_summary()
    {
        Assert.Empty(Cadence.Summarise([]));
    }

    [Fact]
    public void one_beacon_names_why_no_rate_exists()
    {
        var lines = Cadence.Summarise([new Cadence.BeatSample(3.0, 42)]);
        Assert.Contains(lines, line => line.Contains("one beacon", StringComparison.Ordinal));
        Assert.DoesNotContain(lines, line => line.Contains("mean period", StringComparison.Ordinal));
    }

    [Fact]
    public void the_mean_period_is_span_over_beats()
    {
        var lines = Cadence.Summarise(
        [
            new Cadence.BeatSample(2.0, 10),
            new Cadence.BeatSample(7.0, 12),
            new Cadence.BeatSample(12.0, 20),
        ]);
        // (12.0 - 2.0) s over (20 - 10) beats = exactly 1.000000 s.
        Assert.Contains(lines, line => line.Contains("1.000000 s", StringComparison.Ordinal));
        Assert.Contains(lines, line => line.Contains("dseq=10", StringComparison.Ordinal));
    }

    [Fact]
    public void the_summary_owns_its_own_caveat()
    {
        var lines = Cadence.Summarise(
        [
            new Cadence.BeatSample(0.0, 1),
            new Cadence.BeatSample(10.0, 11),
        ]);
        // The caution is part of the output, not a doc comment: whoever pastes
        // the summary into a Report pastes the boundary of what it measures.
        Assert.Contains(lines, line => line.Contains("host", StringComparison.OrdinalIgnoreCase)
            && line.Contains("jitter", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void a_backwards_sequence_is_a_reboot_and_no_rate_is_computed()
    {
        var lines = Cadence.Summarise(
        [
            new Cadence.BeatSample(1.0, 500),
            new Cadence.BeatSample(2.0, 501),
            new Cadence.BeatSample(3.0, 0),
            new Cadence.BeatSample(4.0, 1),
        ]);
        Assert.Contains(lines, line => line.Contains("reboot", StringComparison.OrdinalIgnoreCase));
        Assert.DoesNotContain(lines, line => line.Contains("mean period", StringComparison.Ordinal));
    }

    [Fact]
    public void a_frozen_sequence_yields_a_refusal_not_a_division()
    {
        var lines = Cadence.Summarise(
        [
            new Cadence.BeatSample(1.0, 7),
            new Cadence.BeatSample(9.0, 7),
        ]);
        Assert.DoesNotContain(lines, line => line.Contains("mean period", StringComparison.Ordinal));
        Assert.Contains(lines, line => line.Contains("did not advance", StringComparison.Ordinal));
    }

    [Fact]
    public void every_live_frame_prints_with_its_arrival_stamp()
    {
        var beacon = Cadence.ArrivalLine(12.345, Beacon(9));
        Assert.StartsWith("  +12.345s", beacon, StringComparison.Ordinal);
        Assert.Contains("TOS64-PRESENT/1", beacon, StringComparison.Ordinal);

        var spoor = Cadence.ArrivalLine(0.5, "SPOORJ01\x01\x02"u8.ToArray());
        Assert.Contains("SPOORJ01", spoor, StringComparison.Ordinal);

        var opaque = Cadence.ArrivalLine(1.5, [0x00, 0x01, 0x02]);
        Assert.Contains("3 octet", opaque, StringComparison.Ordinal);
    }
}
