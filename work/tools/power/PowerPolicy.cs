/// The fail-safe rules for a tool that switches mains power, as pure functions.
///
/// `LE-95`'s owner path states four clauses and this file is three of them (the
/// fourth, the mid-transfer refusal, is `TransferGuard`). They are pure because
/// the alternative is a fail-safe whose only test is a real Raspberry Pi 5 on a
/// real relay — and this project owns one board, which is the whole reason
/// `LE-95` exists.
///
/// **The asymmetry that shapes everything below: `off` is the one state a later
/// session cannot recover from without a human hand.** `on` can be undone by a
/// tool. Failing to reach the plug can be retried. A board left dark ends the
/// bench until somebody walks over to it — which is precisely the stall this
/// tool was built to remove, so a tool that can cause it has made the problem
/// worse rather than better.

/// What a readback said about what was asked for.
public enum Confirmation
{
    /// The plug read back the state that was requested. The only success.
    Confirmed,
    /// The plug read back the OTHER state. The command did not take, and this
    /// is a louder finding than silence: something is switching, just not this.
    Contradicted,
    /// The plug did not say. Never reported as done.
    Unknown,
}

/// How a run ended. Ordered by severity, and `ExitCode` is that order.
public enum PowerOutcome
{
    Done = 0,
    /// A guard refused to act — a bound, or a transfer in flight. Nothing was
    /// switched, which is why this is milder than `Unknown`.
    Refused = 1,
    UsageError = 2,
    /// Something was asked of the plug and the plug did not confirm the result.
    /// Half a success, reported as exactly that.
    Unknown = 3,
    /// The loudest outcome this tool has: the board may be OFF and this tool
    /// could not get it back on. The next session needs a hand on the plug.
    LeftOff = 4,
}

/// How far through a cycle a failure happened. The input to clause 1.
public enum CyclePhase
{
    BeforeOff,
    OffConfirmed,
    /// The off command went out and was not confirmed. Treated as off — see
    /// `OwesRestore`.
    OffUnknown,
    OnRequested,
    OnConfirmed,
}

internal static class PowerPolicy
{
    /// Below a second the Pi 5's supply rails and the relay's own settle time
    /// make it a coin toss whether the SoC saw a reset — and a cycle that did
    /// not reset the board yields a capture of the PREVIOUS boot, which is a
    /// stale-evidence failure with no symptom. `LE-87` cost three power cycles
    /// to a stale image; this is the same class and would cost more, because
    /// nothing would even look wrong.
    internal const int MinOffMs = 1000;

    /// A minute is already far past any reset requirement. Past it, the operator
    /// meant something else — a maintenance window, a typo, a units mistake —
    /// and a tool holding a board dark for an hour because a number said so is
    /// not fail-safe.
    internal const int MaxOffMs = 60_000;

    internal const int MinOnWaitSeconds = 1;

    /// Ten minutes. Long enough for any boot-and-beacon this bench measures,
    /// short enough that a stuck run ends inside a session rather than after it.
    internal const int MaxOnWaitSeconds = 600;

    /// Bounded, per `agent.md` rule 6 — fail-safe over keep-trying. Three
    /// attempts and then a loud `LeftOff`, rather than an unbounded retry that
    /// pins the bench overnight and reports nothing at the end of it.
    internal const int RestoreAttempts = 3;

    /// Refused rather than clamped. A clamp answers a question the operator did
    /// not ask and then reports success — the same reason `gem_receive` refuses
    /// a buffer-size encoding instead of rounding it: a rounded bound is a grant
    /// the argument never made.
    internal static bool OffIntervalIsSane(int milliseconds) =>
        milliseconds >= MinOffMs && milliseconds <= MaxOffMs;

    internal static bool OnWaitIsSane(int seconds) =>
        seconds >= MinOnWaitSeconds && seconds <= MaxOnWaitSeconds;

    /// The readback decides, and nothing else does. Not the status code, not
    /// the command's own response body, not the absence of an exception.
    internal static Confirmation Confirm(PlugState requested, PlugState readback)
    {
        if (readback == PlugState.Unknown) return Confirmation.Unknown;
        return readback == requested ? Confirmation.Confirmed : Confirmation.Contradicted;
    }

    /// Clause 1, as one predicate: after the off and before a confirmed on,
    /// this tool owes the bench a restore no matter what else went wrong.
    ///
    /// `OffUnknown` is the subtle entry. The off command was sent and the plug
    /// did not confirm, so the board MAY be off — and "may be off" has to be
    /// treated as off. Reading an unconfirmed off as "probably nothing
    /// happened, no restore needed" is exactly how a bench ends a session dark.
    internal static bool OwesRestore(CyclePhase phase) => phase switch
    {
        CyclePhase.BeforeOff => false,
        CyclePhase.OffConfirmed => true,
        CyclePhase.OffUnknown => true,
        CyclePhase.OnRequested => true,
        CyclePhase.OnConfirmed => false,
        _ => true,
    };

    internal static PowerOutcome AfterExhaustedRestore() => PowerOutcome.LeftOff;

    /// Severity is the exit code, so "report the worst thing that happened"
    /// needs no second ordering to drift out of step with the first.
    internal static PowerOutcome Worst(PowerOutcome a, PowerOutcome b) =>
        ExitCode(a) >= ExitCode(b) ? a : b;

    internal static int ExitCode(PowerOutcome outcome) => (int)outcome;
}
