using System.Globalization;

/// `LE-95` clause 2: refuse to cycle while a TFTP transfer is in flight.
///
/// The board fetches `kernel8.img` over TFTP from `tos64-netboot` after power
/// comes up. Cutting mains during that fetch leaves the firmware holding a
/// partial image, and the failure that follows looks like a kernel fault rather
/// than like a power cut — a whole class of misdiagnosis, on a bench whose last
/// five instrument failures all wore a device failure's costume.
///
/// The mechanism is deliberately the dumbest one that cannot lie: `netboot`
/// writes a marker file for the duration of a transfer and removes it at the
/// end; `tos64-power` reads it. The two programs are separate, so the format is
/// asserted from BOTH sides in this file — the `LE-80` mirror shape, because a
/// writer and a reader that drift here fail OPEN, and failing open on this seam
/// is a power cut.
public sealed class TransferGuardTests
{
    private static readonly DateTimeOffset Now =
        new(2026, 8, 6, 21, 0, 0, TimeSpan.Zero);

    [Fact]
    public void No_marker_means_no_transfer()
    {
        Assert.Equal(TransferState.Idle, TransferGuard.Assess(null, Now));
    }

    [Fact]
    public void A_fresh_marker_means_a_transfer_is_in_flight()
    {
        var marker = Stamp(Now.AddSeconds(-2), "kernel8.img");
        Assert.Equal(TransferState.InFlight, TransferGuard.Assess(marker, Now));
    }

    /// A marker from a `netboot` that was killed mid-transfer would otherwise
    /// block every future cycle forever, which turns a fail-safe into a bench
    /// that cannot be used. Stale is a THIRD state and is reported as one — not
    /// quietly folded into `Idle`, because "the last server died holding an
    /// image" is worth a line on the operator's screen.
    [Fact]
    public void A_stale_marker_is_stale_and_not_silently_idle()
    {
        var marker = Stamp(Now.AddSeconds(-90), "kernel8.img");
        Assert.Equal(TransferState.Stale, TransferGuard.Assess(marker, Now));
    }

    [Fact]
    public void The_staleness_threshold_is_the_transfer_abandon_time()
    {
        // netboot gives up a block after 5 attempts at a 2 s receive timeout,
        // so 10 s is the longest a live transfer can sit without progress. The
        // threshold is derived from that rather than picked, per the standing
        // no-bench-tuned-constants rule.
        Assert.Equal(10, TransferGuard.StaleAfterSeconds);
        Assert.Equal(TransferState.InFlight, TransferGuard.Assess(Stamp(Now.AddSeconds(-9), "k"), Now));
        Assert.Equal(TransferState.Stale, TransferGuard.Assess(Stamp(Now.AddSeconds(-11), "k"), Now));
    }

    /// A marker written by a clock ahead of this one, or by a future format.
    /// Neither is "no transfer", and on a mains seam the only safe reading of
    /// "I cannot tell" is "do not switch".
    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("garbage")]
    [InlineData("not-a-date kernel8.img")]
    [InlineData("2026-08-06")]
    public void An_unreadable_marker_is_Unknown_and_never_Idle(string marker)
    {
        Assert.Equal(TransferState.Unknown, TransferGuard.Assess(marker, Now));
    }

    [Fact]
    public void A_marker_from_the_future_is_Unknown()
    {
        var marker = Stamp(Now.AddSeconds(30), "kernel8.img");
        Assert.Equal(TransferState.Unknown, TransferGuard.Assess(marker, Now));
    }

    // ---- the decision, which is the part that touches mains ---------------

    [Theory]
    [InlineData(TransferState.Idle, true)]
    [InlineData(TransferState.Stale, true)]
    [InlineData(TransferState.InFlight, false)]
    [InlineData(TransferState.Unknown, false)]
    public void Only_Idle_and_Stale_may_cycle(TransferState state, bool mayCycle)
    {
        Assert.Equal(mayCycle, TransferGuard.MayCycle(state));
    }

    /// The guard covers `off` and `cycle` and NOT `on`. Switching a board on
    /// mid-transfer is not a thing that can happen (it is already on if it is
    /// fetching) and, more to the point, `on` is the recovery action clause 1
    /// depends on — a guard that could refuse it would be a fail-safe that can
    /// refuse to be safe.
    [Fact]
    public void The_guard_never_stands_between_the_board_and_power_on()
    {
        Assert.True(TransferGuard.AppliesTo(PlugAction.Off));
        Assert.False(TransferGuard.AppliesTo(PlugAction.On));
        Assert.False(TransferGuard.AppliesTo(PlugAction.Read));
    }

    // ---- the mirror: netboot writes what power reads ----------------------

    /// Without this test the writer's format lives in one program and the
    /// reader's in another, and the two can drift with no symptom until an
    /// operator is standing over a board that got its power cut halfway
    /// through an image. Falsified in both directions below.
    [Fact]
    public void The_beacon_netboot_writes_is_the_marker_power_reads()
    {
        var written = TransferBeacon.Line("kernel8.img", Now);
        Assert.Equal(TransferState.InFlight, TransferGuard.Assess(written, Now));
        Assert.Equal(TransferState.Stale, TransferGuard.Assess(written, Now.AddSeconds(30)));
    }

    [Fact]
    public void The_beacon_names_the_file_so_the_refusal_can_say_which()
    {
        var written = TransferBeacon.Line("kernel8.img", Now);
        Assert.Contains("kernel8.img", TransferGuard.Describe(written));
    }

    /// The two ends agree on the FILE NAME too, not just on the timestamp —
    /// this is the drift the mirror is really guarding, since a reader that
    /// parses the stamp and ignores the rest would pass the test above while
    /// silently accepting a format the writer no longer emits.
    [Fact]
    public void A_beacon_in_the_writers_own_words_round_trips()
    {
        var written = TransferBeacon.Line("a name with spaces.img", Now);
        Assert.Equal(TransferState.InFlight, TransferGuard.Assess(written, Now));
        Assert.Contains("a name with spaces.img", TransferGuard.Describe(written));
    }

    /// The marker's NAME is the third thing that can drift, and it is the one
    /// that fails most quietly: a reader looking for a file the writer stopped
    /// creating sees `Idle` forever and cycles mains through every transfer.
    [Fact]
    public void Both_ends_look_for_the_same_file()
    {
        Assert.Equal(TransferBeacon.MarkerName, TransferGuard.MarkerName);
    }

    private static string Stamp(DateTimeOffset at, string name) =>
        at.ToUniversalTime().ToString("o", CultureInfo.InvariantCulture) + "\t" + name;
}
