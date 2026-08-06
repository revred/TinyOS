// tos64-power — the tenth stage of the board evidence loop.
//
// WHY THIS EXISTS, and it is not convenience. As of 2026-08-06 nine of the ten
// stages of this project's board evidence loop are built and proven: image
// build, staged-digest verification, netboot service, a log readable live,
// waiting for a board event, envelope capture, transmitting to the board,
// parse-meas, and the timing gate. The tenth is A HUMAN HAND ON A MAINS PLUG.
// Because a session cannot power-cycle the board, every session ends at the
// last thing a laptop can do — so release-gate evidence did not move for
// forty-eight hours while every local gate stayed green (`LE-95`).
//
// THE DEVICE REQUIREMENT, which is one line and is a containment requirement
// rather than a preference: the plug must be controllable over the LAN with no
// vendor cloud account. A bench whose board cannot be rebooted while somebody
// else's service is down is a new instrument failure, and this project has had
// five in a row. Tasmota, ESPHome and both Shelly HTTP generations qualify;
// a cloud-only plug does not, at any price.
//
// THE FOUR RULES IT WILL NOT BEND, because this drives mains power and the
// priority ordering (safety before security before correctness before
// performance) does not bend for convenience:
//
//   1. It never leaves the board off on any error path. `off` is the single
//      state a later session cannot recover from without a hand on the plug —
//      which is the very stall this tool exists to remove.
//   2. It refuses to cycle while a TFTP transfer is in flight, and refuses when
//      it cannot tell.
//   3. The off-interval and the on-wait are bounded, and out-of-range values
//      are REFUSED rather than clamped.
//   4. A plug that does not confirm its new state from a readback is reported
//      as UNKNOWN, never as done. Half a success reported as a success is this
//      bench's signature failure and it has happened five times; this is
//      `LE-87`'s lesson applied before the defect rather than after it.
//
// Usage:
//   tos64-power --plug http://10.0.0.9 --dialect tasmota status
//   tos64-power --plug http://10.0.0.9 --dialect tasmota on
//   tos64-power --plug http://10.0.0.9 --dialect tasmota off   [--root <dir>]
//   tos64-power --plug http://10.0.0.9 --dialect tasmota cycle [--off-ms 5000]
//                                                              [--on-wait 20]
//                                                              [--root <dir>]
//
// Exit codes, distinct on purpose so a script cannot mistake one for another:
//   0 done and confirmed   1 refused by a guard   2 usage
//   3 UNKNOWN — asked, not confirmed              4 THE BOARD MAY BE OFF

internal static class Program
{
    private static async Task<int> Main(string[] args)
    {
        // `LE-90`, first statement and nowhere else. Redirected stdout in .NET
        // is buffered at 4 KiB and flushed on exit, and a `cycle --on-wait 600`
        // does not exit for ten minutes — an operator tailing this tool's log
        // would see the banner and then silence while the board sat dark.
        Console.SetOut(new StreamWriter(Console.OpenStandardOutput()) { AutoFlush = true });
        Console.SetError(new StreamWriter(Console.OpenStandardError()) { AutoFlush = true });

        string? plugUrl = null, dialect = null, entity = null, command = null;
        var root = ".";
        var offMs = 5000;
        var onWait = 20;
        var timeoutSeconds = 5;

        for (var i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--plug" when i + 1 < args.Length: plugUrl = args[++i]; break;
                case "--dialect" when i + 1 < args.Length: dialect = args[++i]; break;
                case "--entity" when i + 1 < args.Length: entity = args[++i]; break;
                case "--root" when i + 1 < args.Length: root = args[++i]; break;
                case "--off-ms" when i + 1 < args.Length && int.TryParse(args[i + 1], out var o):
                    offMs = o; i++; break;
                case "--on-wait" when i + 1 < args.Length && int.TryParse(args[i + 1], out var w):
                    onWait = w; i++; break;
                case "--timeout" when i + 1 < args.Length && int.TryParse(args[i + 1], out var t):
                    timeoutSeconds = t; i++; break;
                case "-h" or "--help": Usage(); return 0;
                default:
                    if (!args[i].StartsWith('-')) command ??= args[i];
                    break;
            }
        }

        if (command is null) { Usage(); return Fail(PowerOutcome.UsageError, "no command given"); }

        var kind = PlugDialect.ParseKind(dialect);
        if (kind is null)
        {
            return Fail(PowerOutcome.UsageError,
                $"--dialect must be one of tasmota, shelly-gen1, shelly-gen2, esphome (got \"{dialect}\")");
        }

        var plug = PlugBase.Parse(plugUrl, entity, kind.Value);
        if (plug is null)
        {
            return Fail(PowerOutcome.UsageError, kind == PlugKind.Esphome
                ? "--plug must be an absolute http:// or https:// URL and --entity must name the switch"
                : "--plug must be an absolute http:// or https:// URL (a bare host is refused, deliberately)");
        }

        var client = new PlugClient(TimeSpan.FromSeconds(timeoutSeconds));
        var markerPath = Path.Combine(root, TransferGuard.MarkerName);

        return command switch
        {
            "status" => Report(await ReadBack(client, kind.Value, plug)),
            "on" => await Switch(client, kind.Value, plug, PlugAction.On, markerPath),
            "off" => await Switch(client, kind.Value, plug, PlugAction.Off, markerPath),
            "cycle" => await Cycle(client, kind.Value, plug, markerPath, offMs, onWait),
            _ => Fail(PowerOutcome.UsageError, $"unknown command \"{command}\""),
        };
    }

    // ---- the commands -----------------------------------------------------

    private static int Report(PlugState state)
    {
        Console.WriteLine($"plug: {Name(state)}");
        // `status` reports what it read, and an unreadable plug is not a
        // successful status check — the operator ran this to LEARN the state.
        return PowerPolicy.ExitCode(state == PlugState.Unknown ? PowerOutcome.Unknown : PowerOutcome.Done);
    }

    private static async Task<int> Switch(
        PlugClient client, PlugKind kind, PlugBase plug, PlugAction action, string markerPath)
    {
        if (TransferGuard.AppliesTo(action))
        {
            var refusal = GuardCycle(markerPath);
            if (refusal is not null) return refusal.Value;
        }

        var (outcome, _) = await Command(client, kind, plug, action);
        return PowerPolicy.ExitCode(outcome);
    }

    /// Off, wait, on — with clause 1 wrapped around everything after the off.
    private static async Task<int> Cycle(
        PlugClient client, PlugKind kind, PlugBase plug, string markerPath, int offMs, int onWait)
    {
        if (!PowerPolicy.OffIntervalIsSane(offMs))
        {
            return Fail(PowerOutcome.Refused,
                $"--off-ms {offMs} is outside [{PowerPolicy.MinOffMs}, {PowerPolicy.MaxOffMs}]; " +
                "refused rather than clamped");
        }
        if (!PowerPolicy.OnWaitIsSane(onWait))
        {
            return Fail(PowerOutcome.Refused,
                $"--on-wait {onWait} is outside [{PowerPolicy.MinOnWaitSeconds}, " +
                $"{PowerPolicy.MaxOnWaitSeconds}]; refused rather than clamped");
        }

        var refusal = GuardCycle(markerPath);
        if (refusal is not null) return refusal.Value;

        var phase = CyclePhase.BeforeOff;
        var worst = PowerOutcome.Done;
        try
        {
            var (offOutcome, offConfirmation) = await Command(client, kind, plug, PlugAction.Off);
            // Both an unconfirmed off and a confirmed one owe a restore: the
            // board MAY be off, and "may be off" is treated as off.
            phase = offConfirmation == Confirmation.Confirmed
                ? CyclePhase.OffConfirmed
                : CyclePhase.OffUnknown;
            worst = PowerPolicy.Worst(worst, offOutcome);

            Console.WriteLine($"  holding off for {offMs} ms");
            await Task.Delay(offMs);

            phase = CyclePhase.OnRequested;
            var (onOutcome, onConfirmation) = await Command(client, kind, plug, PlugAction.On);
            if (onConfirmation == Confirmation.Confirmed) phase = CyclePhase.OnConfirmed;
            worst = PowerPolicy.Worst(worst, onOutcome);
        }
        catch (Exception e)
        {
            // Nothing in the loop above is expected to throw — `PlugClient`
            // returns its failures — but clause 1 is not conditional on that
            // being true, because the whole point of a fail-safe is the path
            // nobody predicted.
            Console.Error.WriteLine($"tos64-power: unexpected: {e.Message}");
            worst = PowerPolicy.Worst(worst, PowerOutcome.Unknown);
        }

        if (PowerPolicy.OwesRestore(phase))
        {
            worst = PowerPolicy.Worst(worst, await Restore(client, kind, plug));
        }
        else
        {
            Console.WriteLine($"  board is ON; giving it {onWait}s before anything reads the wire");
            await Task.Delay(TimeSpan.FromSeconds(onWait));
        }

        return PowerPolicy.ExitCode(worst);
    }

    /// Clause 1's teeth: bounded attempts to leave the board ON, and a loud,
    /// distinct exit code if they run out.
    private static async Task<PowerOutcome> Restore(PlugClient client, PlugKind kind, PlugBase plug)
    {
        for (var attempt = 1; attempt <= PowerPolicy.RestoreAttempts; attempt++)
        {
            Console.Error.WriteLine(
                $"tos64-power: the board is not confirmed ON — restore attempt " +
                $"{attempt}/{PowerPolicy.RestoreAttempts}");
            var (_, confirmation) = await Command(client, kind, plug, PlugAction.On);
            if (confirmation == Confirmation.Confirmed) return PowerOutcome.Unknown;
            await Task.Delay(TimeSpan.FromSeconds(2));
        }

        Console.Error.WriteLine();
        Console.Error.WriteLine("tos64-power: THE BOARD MAY BE OFF AND THIS TOOL COULD NOT SWITCH IT ON.");
        Console.Error.WriteLine("             The next session needs a hand on the plug. This is exit 4,");
        Console.Error.WriteLine("             and it is deliberately not the same code as any other failure.");
        return PowerPolicy.AfterExhaustedRestore();
    }

    /// One command plus the readback that is allowed to confirm it.
    ///
    /// The command's own response body is never consulted — Shelly Gen2's
    /// `Switch.Set` answers with the PREVIOUS state, so a tool that read it as
    /// confirmation would report a relay that did nothing as done.
    private static async Task<(PowerOutcome Outcome, Confirmation Confirmation)> Command(
        PlugClient client, PlugKind kind, PlugBase plug, PlugAction action)
    {
        var wanted = action == PlugAction.On ? PlugState.On : PlugState.Off;
        var reply = await client.Fetch(PlugDialect.Request(kind, plug, action));
        if (!reply.Reached)
        {
            Console.Error.WriteLine($"tos64-power: {action} not delivered: {reply.Failure}");
            return (PowerOutcome.Unknown, Confirmation.Unknown);
        }
        if (!reply.Ok)
        {
            Console.Error.WriteLine($"tos64-power: {action} refused by the plug: {reply.Failure}");
        }

        var readback = await ReadBack(client, kind, plug);
        var confirmation = PowerPolicy.Confirm(wanted, readback);
        switch (confirmation)
        {
            case Confirmation.Confirmed:
                Console.WriteLine($"  {action}: confirmed by readback ({Name(readback)})");
                return (PowerOutcome.Done, confirmation);
            case Confirmation.Contradicted:
                // Louder than silence: something is switching, and it is not
                // this. A wrong socket looks exactly like this.
                Console.Error.WriteLine(
                    $"tos64-power: {action} was sent and the plug reads back {Name(readback)}. " +
                    "CHECK WHICH SOCKET THIS PLUG IS IN.");
                return (PowerOutcome.Unknown, confirmation);
            default:
                Console.Error.WriteLine(
                    $"tos64-power: {action} was sent and the plug did not confirm. State UNKNOWN, " +
                    "which is not a smaller success.");
                return (PowerOutcome.Unknown, confirmation);
        }
    }

    private static async Task<PlugState> ReadBack(PlugClient client, PlugKind kind, PlugBase plug)
    {
        var reply = await client.Fetch(PlugDialect.Request(kind, plug, PlugAction.Read));
        return reply.Reached ? PlugDialect.ReadState(kind, reply.Body ?? "") : PlugState.Unknown;
    }

    // ---- clause 2 ---------------------------------------------------------

    /// Null to proceed, or the exit code to return.
    private static int? GuardCycle(string markerPath)
    {
        var marker = TransferGuard.Read(markerPath);
        var state = TransferGuard.Assess(marker, DateTimeOffset.UtcNow);
        if (TransferGuard.MayCycle(state))
        {
            if (state == TransferState.Stale)
            {
                Console.WriteLine(
                    $"  a stale transfer marker is present ({TransferGuard.Describe(marker)}); " +
                    "a previous tos64-netboot died mid-transfer. Proceeding.");
            }
            return null;
        }

        Console.Error.WriteLine($"tos64-power: REFUSED — {TransferGuard.Describe(marker)}");
        Console.Error.WriteLine("             Cutting mains mid-TFTP leaves the firmware holding a partial");
        Console.Error.WriteLine("             image, and what follows looks like a kernel fault rather than");
        Console.Error.WriteLine($"             like a power cut. Marker: {markerPath}");
        return PowerPolicy.ExitCode(PowerOutcome.Refused);
    }

    // ---- plumbing ---------------------------------------------------------

    private static string Name(PlugState state) => state switch
    {
        PlugState.On => "ON",
        PlugState.Off => "OFF",
        _ => "UNKNOWN",
    };

    private static int Fail(PowerOutcome outcome, string message)
    {
        Console.Error.WriteLine($"tos64-power: {message}");
        return PowerPolicy.ExitCode(outcome);
    }

    private static void Usage()
    {
        Console.WriteLine("tos64-power — LAN-controlled mains for the Pi 5 bench (LE-95)");
        Console.WriteLine();
        Console.WriteLine("  tos64-power --plug <url> --dialect <kind> [--entity <id>] <command>");
        Console.WriteLine();
        Console.WriteLine("Commands:");
        Console.WriteLine("  status                 read the plug's state and say so");
        Console.WriteLine("  on                     switch on, confirmed by readback");
        Console.WriteLine("  off                    switch off, confirmed by readback");
        Console.WriteLine("  cycle                  off, hold, on — and NEVER leaves the board off");
        Console.WriteLine();
        Console.WriteLine("Options:");
        Console.WriteLine("  --dialect tasmota | shelly-gen1 | shelly-gen2 | esphome");
        Console.WriteLine("  --entity <id>          ESPHome only; the switch entity, never guessed");
        Console.WriteLine($"  --off-ms <n>           hold-off, [{PowerPolicy.MinOffMs}, {PowerPolicy.MaxOffMs}], refused if outside");
        Console.WriteLine($"  --on-wait <s>          settle after on, [{PowerPolicy.MinOnWaitSeconds}, {PowerPolicy.MaxOnWaitSeconds}]");
        Console.WriteLine("  --root <dir>           the tftp root, so the transfer marker can be found");
        Console.WriteLine("  --timeout <s>          per-request deadline (default 5)");
        Console.WriteLine();
        Console.WriteLine("Exit: 0 done  1 refused  2 usage  3 UNKNOWN  4 THE BOARD MAY BE OFF");
        Console.WriteLine();
        Console.WriteLine("The plug must be controllable on the LAN with no vendor cloud account.");
        Console.WriteLine("That is a containment requirement, not a preference: a bench that cannot");
        Console.WriteLine("reboot its board while someone else's service is down is a new instrument");
        Console.WriteLine("failure, and this project has had five.");
    }
}
