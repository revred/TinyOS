using System.Globalization;

/// A marker file that says "an image is on the wire right now".
///
/// It exists for one reader: `tos64-power`, which switches mains on the Pi 5's
/// supply and must refuse to cut power mid-transfer (`LE-95` clause 2). A power
/// cut during the TFTP fetch leaves the firmware holding a partial `kernel8.img`
/// and what follows looks like a kernel fault rather than like a power cut —
/// which is this bench's most expensive failure shape, an instrument failure in
/// a device failure's costume, now six deep.
///
/// The format is one line, tab separated: an ISO-8601 round-trip UTC stamp and
/// the file being served. Nothing else, because the two ends live in two
/// programs and every field is a chance for the writer and the reader to drift.
/// `power.tests` asserts this writer's output against `TransferGuard`'s reader
/// from both directions — the `LE-80` mirror shape — because drift here fails
/// OPEN, and failing open on this seam is the power cut.
///
/// Best-effort by construction: if the marker cannot be written, the transfer
/// still happens. This is a guard against a power cycle, not a lock on the
/// image, and a netboot server that refused to serve because it could not
/// create a dotfile would have traded a rare failure for a certain one.
internal static class TransferBeacon
{
    /// The line, as a pure function, so the format has one definition and the
    /// mirror test can hold it against the reader without touching a disk.
    internal static string Line(string file, DateTimeOffset at) =>
        at.ToUniversalTime().ToString("o", CultureInfo.InvariantCulture) + "\t" + file;

    /// Refreshed on every acknowledged block rather than written once at the
    /// start. A single write at the start would make a slow transfer look stale
    /// after ten seconds and let `tos64-power` cycle straight through the middle
    /// of it — the marker has to say "still progressing", not "began".
    internal static void Mark(string root, string file)
    {
        try
        {
            File.WriteAllText(Path.Combine(root, MarkerName), Line(file, DateTimeOffset.UtcNow));
        }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
    }

    internal static void Clear(string root)
    {
        try
        {
            File.Delete(Path.Combine(root, MarkerName));
        }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
    }

    /// Duplicated by value in `TransferGuard.MarkerName` and asserted equal by
    /// the mirror test, because the alternative is a shared assembly between a
    /// DHCP server and a mains switch.
    internal const string MarkerName = ".tos64-transfer";
}
