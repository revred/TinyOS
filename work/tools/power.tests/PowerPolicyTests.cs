/// The fail-safe rules, as pure functions, because this seam is mains power.
///
/// `LE-95`'s owner path is a four-clause contract and every clause is here:
///
///   1. never leave the board off on any error path
///   2. refuse to cycle while a transfer is in flight   (`TransferGuardTests`)
///   3. bound the off-interval and the on-wait
///   4. a plug that does not confirm from a readback is UNKNOWN, never done
///
/// Clause 1 is the one that has no second chance. `off` is the single state a
/// subsequent session cannot recover from without a human hand on the plug —
/// which is the entire defect `LE-95` was raised over — so a run that fails
/// after switching off has NOT failed safely until the board is back on.
public sealed class PowerPolicyTests
{
    // ---- clause 3: bounds, refused rather than rounded ---------------------

    /// Refused rather than clamped, deliberately. A clamp answers a question
    /// the operator did not ask and then reports success; `gem_receive`'s
    /// buffer-size encoding made the same choice for the same reason — a
    /// rounded bound is a grant the argument did not make.
    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(999)]
    [InlineData(60_001)]
    [InlineData(3_600_000)]
    [InlineData(-1)]
    public void An_off_interval_outside_the_bench_range_is_refused(int offMs)
    {
        Assert.False(PowerPolicy.OffIntervalIsSane(offMs));
    }

    [Theory]
    [InlineData(1000)]
    [InlineData(5000)]
    [InlineData(60_000)]
    public void An_off_interval_inside_the_bench_range_is_accepted(int offMs)
    {
        Assert.True(PowerPolicy.OffIntervalIsSane(offMs));
    }

    /// The lower bound is not a style choice. Below a second the Pi 5's supply
    /// rails and the plug's own relay settle time make it a coin toss whether
    /// the SoC actually saw a reset, and a cycle that did not reset the board
    /// produces a capture of the PREVIOUS boot — a stale-evidence failure with
    /// no symptom, which is this bench's most expensive shape (`LE-87`).
    [Fact]
    public void The_lower_bound_is_a_full_second()
    {
        Assert.False(PowerPolicy.OffIntervalIsSane(PowerPolicy.MinOffMs - 1));
        Assert.True(PowerPolicy.OffIntervalIsSane(PowerPolicy.MinOffMs));
        Assert.Equal(1000, PowerPolicy.MinOffMs);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-5)]
    [InlineData(601)]
    public void An_on_wait_outside_the_bench_range_is_refused(int seconds)
    {
        Assert.False(PowerPolicy.OnWaitIsSane(seconds));
    }

    [Theory]
    [InlineData(1)]
    [InlineData(20)]
    [InlineData(600)]
    public void An_on_wait_inside_the_bench_range_is_accepted(int seconds)
    {
        Assert.True(PowerPolicy.OnWaitIsSane(seconds));
    }

    // ---- clause 4: confirmation comes from a readback, or not at all -------

    [Theory]
    [InlineData(PlugState.On, PlugState.On, Confirmation.Confirmed)]
    [InlineData(PlugState.Off, PlugState.Off, Confirmation.Confirmed)]
    [InlineData(PlugState.On, PlugState.Off, Confirmation.Contradicted)]
    [InlineData(PlugState.Off, PlugState.On, Confirmation.Contradicted)]
    [InlineData(PlugState.On, PlugState.Unknown, Confirmation.Unknown)]
    [InlineData(PlugState.Off, PlugState.Unknown, Confirmation.Unknown)]
    public void A_readback_decides_and_nothing_else_does(
        PlugState requested, PlugState readback, Confirmation expected)
    {
        Assert.Equal(expected, PowerPolicy.Confirm(requested, readback));
    }

    /// Unknown is not a smaller success. It is reported with its own exit code
    /// so a script cannot mistake it for one, and the code is distinct from
    /// both "refused" and "done" — the same reason `xtask` distinguishes
    /// `BoardSilent` from `KernelBootFailed`.
    [Theory]
    [InlineData(PowerOutcome.Done, 0)]
    [InlineData(PowerOutcome.Refused, 1)]
    [InlineData(PowerOutcome.UsageError, 2)]
    [InlineData(PowerOutcome.Unknown, 3)]
    [InlineData(PowerOutcome.LeftOff, 4)]
    public void Every_outcome_has_its_own_exit_code(PowerOutcome outcome, int code)
    {
        Assert.Equal(code, PowerPolicy.ExitCode(outcome));
    }

    [Fact]
    public void No_two_outcomes_share_an_exit_code()
    {
        var codes = Enum.GetValues<PowerOutcome>().Select(PowerPolicy.ExitCode).ToArray();
        Assert.Equal(codes.Length, codes.Distinct().Count());
    }

    // ---- clause 1: never leave the board off ------------------------------

    /// The invariant stated as a function over the whole cycle, so it can be
    /// checked without a relay: whatever went wrong, if the last thing this
    /// tool did was switch power off, the run is not finished.
    [Theory]
    [InlineData(CyclePhase.BeforeOff, false)]
    [InlineData(CyclePhase.OffConfirmed, true)]
    [InlineData(CyclePhase.OffUnknown, true)]
    [InlineData(CyclePhase.OnRequested, true)]
    [InlineData(CyclePhase.OnConfirmed, false)]
    public void A_failure_after_the_off_always_owes_a_restore(CyclePhase phase, bool owes)
    {
        Assert.Equal(owes, PowerPolicy.OwesRestore(phase));
    }

    /// `OffUnknown` is the subtle one. The off command was sent and the plug
    /// did not confirm — so the board may be off, and "may be off" has to be
    /// treated as off. Reading an unconfirmed off as "probably nothing
    /// happened, no restore needed" is precisely how a bench ends a session
    /// dark.
    [Fact]
    public void An_unconfirmed_off_is_treated_as_off()
    {
        Assert.True(PowerPolicy.OwesRestore(CyclePhase.OffUnknown));
    }

    /// Bounded, per `agent.md` rule 6: fail-safe over keep-trying. Three
    /// attempts and then a loud `LeftOff`, rather than an unbounded retry loop
    /// that pins a bench overnight and reports nothing.
    [Fact]
    public void Restore_is_bounded_and_then_says_so()
    {
        Assert.Equal(3, PowerPolicy.RestoreAttempts);
        Assert.Equal(PowerOutcome.LeftOff, PowerPolicy.AfterExhaustedRestore());
    }

    /// The loudest outcome in the tool, and it is not the same as a plug that
    /// merely failed to answer: `LeftOff` means the next session needs a hand.
    [Fact]
    public void LeftOff_outranks_every_other_outcome()
    {
        Assert.Equal(PowerOutcome.LeftOff, PowerPolicy.Worst(PowerOutcome.LeftOff, PowerOutcome.Unknown));
        Assert.Equal(PowerOutcome.LeftOff, PowerPolicy.Worst(PowerOutcome.Done, PowerOutcome.LeftOff));
        Assert.Equal(PowerOutcome.Unknown, PowerPolicy.Worst(PowerOutcome.Done, PowerOutcome.Unknown));
        Assert.Equal(PowerOutcome.Refused, PowerPolicy.Worst(PowerOutcome.Done, PowerOutcome.Refused));
    }
}
