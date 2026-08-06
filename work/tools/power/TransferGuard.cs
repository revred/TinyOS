using System.Globalization;

/// `LE-95` clause 2: never cut mains while `tos64-netboot` is serving an image.
///
/// The Pi 5 firmware fetches `kernel8.img` over TFTP after power comes up. A
/// power cut during that fetch leaves the firmware holding a partial image, and
/// what follows looks like a kernel fault rather than like a power cut — a
/// misdiagnosis this bench is well practised at buying, since the last five
/// instrument failures all arrived wearing a device failure's costume.
///
/// The mechanism is the dumbest one that cannot lie: `tos64-netboot` writes a
/// marker file for the duration of each transfer (`TransferBeacon`) and removes
/// it at the end; this reads it. Two programs, one format — so the format is
/// asserted from both sides in `power.tests`, because a writer and a reader
/// that drift here fail OPEN, and failing open on this seam is the power cut.
public enum TransferState
{
    /// No marker. Nothing is being served.
    Idle,
    /// A marker, recently written. An image is on the wire right now.
    InFlight,
    /// A marker left behind by a `netboot` that died mid-transfer. Safe to
    /// switch — and named rather than folded into `Idle`, because "the last
    /// server died holding an image" is worth a line on the operator's screen.
    Stale,
    /// A marker that could not be read: a future format, a clock ahead of this
    /// one, a truncated write. On a mains seam the only safe reading of "I
    /// cannot tell" is "do not switch".
    Unknown,
}

internal static class TransferGuard
{
    /// Derived, not tuned. `tos64-netboot` gives up a block after five attempts
    /// at a two-second receive timeout, so ten seconds is the longest a LIVE
    /// transfer can sit without progress. Picking a round number here instead
    /// would be a bench-tuned constant, which is a standing prohibition in this
    /// repository, and it would be wrong in both directions: too short cuts
    /// power on a slow transfer, too long blocks a bench behind a dead server.
    internal const int StaleAfterSeconds = 10;

    /// The default marker path, beside the served root so it travels with the
    /// bench directory the operator is already thinking about.
    internal const string MarkerName = ".tos64-transfer";

    /// Total, non-throwing, and `Idle` only for the one input that means it.
    internal static TransferState Assess(string? marker, DateTimeOffset now)
    {
        if (marker is null) return TransferState.Idle;

        var stamp = StampOf(marker);
        if (stamp is null) return TransferState.Unknown;

        var age = now - stamp.Value;
        // A marker from the future is a clock disagreement, and this tool does
        // not get to decide which clock is right while holding a mains switch.
        if (age < TimeSpan.Zero) return TransferState.Unknown;
        return age.TotalSeconds <= StaleAfterSeconds ? TransferState.InFlight : TransferState.Stale;
    }

    /// What the operator is told, naming the file so the refusal says WHICH
    /// image is on the wire rather than merely that something is.
    internal static string Describe(string? marker)
    {
        if (marker is null) return "no transfer in flight";
        var stamp = StampOf(marker);
        if (stamp is null) return $"unreadable transfer marker: \"{marker.Trim()}\"";
        return $"{FileOf(marker)} last progressed {stamp.Value:O}";
    }

    /// The guard covers `off` and the off leg of `cycle`, and NOT `on`.
    /// Switching a board on mid-transfer is not a thing that can happen — it is
    /// already powered if it is fetching — and, more to the point, `on` is the
    /// recovery action clause 1 depends on. A guard able to refuse it would be
    /// a fail-safe that can refuse to be safe.
    internal static bool AppliesTo(PlugAction action) => action == PlugAction.Off;

    internal static bool MayCycle(TransferState state) =>
        state is TransferState.Idle or TransferState.Stale;

    /// The marker as it sits on disk, or null if there is none. An unreadable
    /// file is NOT the same as an absent one: it comes back as an empty string,
    /// which `Assess` reads as `Unknown` and refuses on.
    internal static string? Read(string path)
    {
        try
        {
            return File.Exists(path) ? File.ReadAllText(path) : null;
        }
        catch (IOException)
        {
            return "";
        }
        catch (UnauthorizedAccessException)
        {
            return "";
        }
    }

    private static DateTimeOffset? StampOf(string marker)
    {
        var field = marker.Split('\t')[0].Trim();
        if (field.Length == 0) return null;

        // `TryParseExact` on the round-trip form, not `TryParse`. A lenient
        // parse accepts `2026-08-06` — a bare date, which is what a HALF-WRITTEN
        // marker looks like — and hands back midnight, twenty-one hours stale,
        // so the guard would wave through a transfer that is in flight right
        // now. The one input class this function must not be generous with is
        // the one that means "I could not tell".
        return DateTimeOffset.TryParseExact(
            field, "o", CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind, out var parsed)
            ? parsed
            : null;
    }

    private static string FileOf(string marker)
    {
        var fields = marker.Split('\t');
        return fields.Length > 1 ? fields[1].Trim() : "(unnamed transfer)";
    }
}
