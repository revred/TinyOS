using System.Security.Cryptography;

/// What a served file's log line says.
///
/// LE-87's second half. The stale server on 2026-08-06 logged `-> serving
/// 262144 bytes` and that line was TRUE — it just said nothing about WHICH
/// bytes, from which root, so the operator had no way to compare what was
/// served against what had been built. A digest and an absolute path turn a
/// three-power-cycle diagnosis into reading one line.
internal static class TransferLog
{
    /// The full digest, not a prefix. The question this line exists to answer
    /// is "is this the image I just built?", and that is a comparison against a
    /// digest the operator has in another window — where a truncated one is
    /// merely awkward to check and buys a shorter line nobody wanted.
    internal static string Served(string path, byte[] bytes) =>
        $"  -> serving {bytes.Length} bytes  sha256={Convert.ToHexStringLower(SHA256.HashData(bytes))}\n" +
        $"     from {path}";
}
