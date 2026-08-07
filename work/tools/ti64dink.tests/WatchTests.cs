// The `--until` conditions — the watch that once reported a live event as an
// absence for 300 seconds (LE-80). Every rule here exists because its absence
// cost a board session: the rung fires only when the id AND the declared
// (category, action) pair agree, a retained frame never fires, and an
// undeclared epoch is an absence rather than boot zero.

using System.Text;

public sealed class WatchTests
{
    [Fact]
    public void an_unknown_condition_or_rung_is_refused_at_parse_never_guessed()
    {
        Assert.Null(Program.Watch.Parse("rung=Nonsense"));
        Assert.Null(Program.Watch.Parse("something-else"));
        Assert.Null(Program.Watch.Parse("text="));
        Assert.NotNull(Program.Watch.Parse("rung=DispatchRound"));
        Assert.NotNull(Program.Watch.Parse("epoch-change"));
    }

    [Fact]
    public void the_rung_watch_fires_on_id_and_pair_agreement()
    {
        var watch = Program.Watch.Parse("rung=DispatchRound")!;
        var payload = DecodeTests.SpoorFrame(0, 1, 0, DecodeTests.DispatchRound());
        Assert.True(watch.Offer(payload));
        Assert.True(watch.Sighted);
    }

    [Fact]
    public void a_target_masquerading_under_the_wrong_pair_does_not_fire()
    {
        // LE-80's exact hazard, inverted: `target` means something else on
        // other paths (the x86_64 dispatcher puts a task index there). Task
        // index 9 under Scheduling/Select must not read as rung 9.
        var watch = Program.Watch.Parse("rung=DispatchRound")!;
        var masquerade = DecodeTests.Record(0, 0, 4, 0, 9, 0);
        Assert.False(watch.Offer(DecodeTests.SpoorFrame(0, 1, 0, masquerade)));
    }

    [Fact]
    public void a_retained_frame_replays_the_past_and_never_fires_a_watch()
    {
        var watch = Program.Watch.Parse("rung=DispatchRound")!;
        var payload = DecodeTests.SpoorFrame(0, 1, 0x0001, DecodeTests.DispatchRound());
        Assert.False(watch.Offer(payload));
    }

    [Fact]
    public void an_epoch_change_is_a_reboot_and_fires()
    {
        var watch = Program.Watch.Parse("epoch-change")!;
        Assert.False(watch.Offer(DecodeTests.SpoorFrame(0, 5, 0, DecodeTests.DispatchRound())));
        Assert.True(watch.Offer(DecodeTests.SpoorFrame(0, 6, 0, DecodeTests.DispatchRound())));
    }

    [Fact]
    public void an_undeclared_epoch_is_an_absence_not_boot_zero()
    {
        var watch = Program.Watch.Parse("epoch-change")!;
        Assert.False(watch.Offer(DecodeTests.SpoorFrame(0, 0, 0, DecodeTests.DispatchRound())));
        Assert.False(watch.Offer(DecodeTests.SpoorFrame(1, 5, 0, DecodeTests.DispatchRound())));
        // 0 -> 5 is a declaration arriving, not a change between two boots.
        Assert.False(watch.Sighted);
    }

    [Fact]
    public void the_text_watch_fires_on_a_harvested_substring()
    {
        var watch = Program.Watch.Parse("text=STATE=BEACONING")!;
        var quiet = Encoding.ASCII.GetBytes("TOS64-BEAT/1 park=1 STATE=STOPPED\0");
        var loud = Encoding.ASCII.GetBytes("TOS64-BEAT/1 park=2 STATE=BEACONING\0");
        Assert.False(watch.Offer(quiet));
        Assert.True(watch.Offer(loud));
    }
}
