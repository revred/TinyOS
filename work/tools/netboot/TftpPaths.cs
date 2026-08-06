/// Turning a TFTP request name into a file under the served root, or into a
/// refusal.
///
/// This is one function because it has to be one decision. Splitting
/// "normalise" from "guard" is how LE-88 happened: the guard was correct and
/// the thing handed to it was not, and each half looked right on its own.
internal static class TftpPaths
{
    /// The absolute path of the requested file, or `null` if the request
    /// escapes the root.
    ///
    /// A TFTP name is ALWAYS root-relative — the protocol has no concept of an
    /// absolute path, so a leading slash is a client's way of spelling "the
    /// root", not an attempt to escape it. Stripping it is required, not merely
    /// permissive: `Path.Combine(root, "/config.txt")` DISCARDS the root and
    /// yields `C:\config.txt`, which the containment check below then correctly
    /// refuses — so the naive combine turns a legitimate request into an access
    /// violation.
    ///
    /// That cost a board run on 2026-08-06 (LE-88). The Pi 5 firmware fetches
    /// `config.txt` bare at one stage and `/config.txt` at a later one; the
    /// bare fetch succeeded, the slashed fetch was REFUSED, so the settings
    /// that stage reads — `pciex4_reset=0` among them — never applied. The
    /// firmware then reset the RP1 PCIe link and TinyOS reported the honest
    /// consequence: confession code 2 (`PhyDown`), detail `0xE080` — RC mode
    /// yes, PHY never trained.
    internal static string? Resolve(string root, string requested)
    {
        if (string.IsNullOrEmpty(requested)) return null;

        var relative = requested.Replace('/', Path.DirectorySeparatorChar)
                                .Replace('\\', Path.DirectorySeparatorChar)
                                .TrimStart(Path.DirectorySeparatorChar);
        if (relative.Length == 0) return null;

        string rootFull, full;
        try
        {
            rootFull = Path.GetFullPath(root);
            full = Path.GetFullPath(Path.Combine(rootFull, relative));
        }
        catch (Exception e) when (e is ArgumentException or NotSupportedException or PathTooLongException)
        {
            return null;
        }

        // Compared WITH the trailing separator, so a sibling directory whose
        // name merely starts with the root's name — `…\pi5-old` against a root
        // of `…\pi5` — is outside rather than inside. `..` is still refused;
        // only the leading-slash spelling of the root was normalised above.
        var prefix = rootFull.EndsWith(Path.DirectorySeparatorChar)
            ? rootFull
            : rootFull + Path.DirectorySeparatorChar;
        return full.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) ? full : null;
    }
}
