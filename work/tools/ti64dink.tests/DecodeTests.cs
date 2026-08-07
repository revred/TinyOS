// The decode and harvest paths — the half of the console that reads.
//
// These paths carried LE-80 (a live rung decoded as an absence) and the
// EnvelopeForParser filter carried the 2026-08-05 verdict-line omission that
// made a perfect envelope parse as a capture problem. Every rule they encode
// was learned against a live board; this file is where the rules stop being
// enforced only by the next board session's confusion.

using System.Text;

public sealed class DecodeTests
{
    internal static byte[] SpoorFrame(ulong seq, uint epoch, ushort flags, params ulong[] records)
    {
        var bytes = new List<byte>();
        bytes.AddRange("SPOORJ01"u8.ToArray());
        bytes.AddRange(BitConverter.GetBytes(seq));
        bytes.AddRange(BitConverter.GetBytes((ushort)records.Length));
        bytes.AddRange(BitConverter.GetBytes(flags));
        bytes.AddRange(BitConverter.GetBytes(epoch));
        foreach (var record in records)
        {
            bytes.AddRange(BitConverter.GetBytes(record));
        }
        return bytes.ToArray();
    }

    internal static ulong Record(int cat, int who, int act, int outcome, int target, uint cost) =>
        ((ulong)(uint)cat << 60) | ((ulong)(uint)who << 56) | ((ulong)(uint)act << 52)
        | ((ulong)(uint)outcome << 48) | ((ulong)(uint)target << 32) | cost;

    /// Category::Dispatch is discriminant 3, Action::Select is 4 — the
    /// DispatchRound record as the board stamps it (rung id 9 in `target`).
    internal static ulong DispatchRound(uint cost = 0) => Record(3, 0, 4, 0, 9, cost);

    [Fact]
    public void a_wellformed_frame_round_trips()
    {
        var frames = Program.DecodeAll(SpoorFrame(7, 0xAB, 0, DispatchRound(2), Record(6, 0, 0, 0, 1, 0)));
        var frame = Assert.Single(frames);
        Assert.Equal(7UL, frame.Seq);
        Assert.Equal(2, frame.Count);
        Assert.Equal(0xABU, frame.Epoch);
        Assert.False(frame.Retained);
        Assert.Equal(9UL, frame.ExpectedNext);
    }

    [Fact]
    public void a_count_beyond_the_wire_bound_is_refused_before_it_sizes_a_read()
    {
        // 182 claimed records against kernel::spoor_wire::MAX_RECORDS = 181,
        // with far fewer bytes present — the field an attacker or a corrupt
        // capture could inflate.
        var bytes = SpoorFrame(0, 1, 0, DispatchRound());
        bytes[16] = 182;
        bytes[17] = 0;
        Assert.Empty(Program.DecodeAll(bytes));
    }

    [Fact]
    public void a_truncated_frame_is_refused_not_partially_decoded()
    {
        var whole = SpoorFrame(0, 1, 0, DispatchRound(), DispatchRound());
        Assert.Empty(Program.DecodeAll(whole[..^4]));
    }

    [Fact]
    public void a_retained_frame_says_so_and_advances_no_expectation()
    {
        var frames = Program.DecodeAll(SpoorFrame(5, 1, 0x0001, DispatchRound()));
        var frame = Assert.Single(frames);
        Assert.True(frame.Retained);
        Assert.Null(frame.ExpectedNext);
    }

    [Fact]
    public void an_envelope_line_is_harvested_once_no_matter_how_often_the_board_cycles_it()
    {
        var line = "TOS64-PRESENT/1 board=pi5-bcm2712 seq=42";
        var payload = Encoding.ASCII.GetBytes(line + "\0\0\0\0");
        var into = new List<string>();
        Program.HarvestText(payload, into);
        Program.HarvestText(payload, into);
        Assert.Equal([line], into);
    }

    [Fact]
    public void short_runs_and_equals_free_runs_are_noise_not_envelope_lines()
    {
        var into = new List<string>();
        // Under MinEnvelopeLine even though it carries '='.
        Program.HarvestText(Encoding.ASCII.GetBytes("TOS64-X=1\0"), into);
        // Long enough, but no key=value shape — the rule is shape, not prefix.
        Program.HarvestText(Encoding.ASCII.GetBytes("JUSTSOMEPRINTABLETEXTWITHNOPAIR\0"), into);
        Assert.Empty(into);
    }

    [Fact]
    public void the_envelope_is_rotated_so_begin_leads_and_nothing_is_reordered()
    {
        var harvested = new List<string>
        {
            "TOS64-MEAS/2 D04 context_switch min=80 p50=90",
            "TOS64-MEAS/2 END metrics=8",
            "TOS64-MEAS/2 BEGIN fixture=measure",
            "TOS64-MEAS/2 D05 dispatch min=70 p50=75",
        };
        var output = Program.EnvelopeForParser(harvested);
        Assert.Equal(
        [
            "TOS64-MEAS/2 BEGIN fixture=measure",
            "TOS64-MEAS/2 D05 dispatch min=70 p50=75",
            "TOS64-MEAS/2 D04 context_switch min=80 p50=90",
            "TOS64-MEAS/2 END metrics=8",
        ], output);
    }

    [Fact]
    public void the_verdict_rides_last_and_qual_lines_ride_deduplicated()
    {
        var harvested = new List<string>
        {
            "TOS64-RESULT/1 fixture=measure ok=true",
            "TOS64-QUAL/1 boot_entry current_el=EL2 raw=0x8 now_at=EL1",
            "TOS64-MEAS/2 BEGIN fixture=measure",
            "TOS64-QUAL/1 boot_entry current_el=EL2 raw=0x8 now_at=EL1",
            "TOS64-MEAS/2 END metrics=0",
        };
        var output = Program.EnvelopeForParser(harvested);
        Assert.Equal(
        [
            "TOS64-MEAS/2 BEGIN fixture=measure",
            "TOS64-MEAS/2 END metrics=0",
            "TOS64-QUAL/1 boot_entry current_el=EL2 raw=0x8 now_at=EL1",
            "TOS64-RESULT/1 fixture=measure ok=true",
        ], output);
    }
}
