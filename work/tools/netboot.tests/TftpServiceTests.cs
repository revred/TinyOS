using System.Security.Cryptography;
using System.Text;

namespace Netboot.Tests;

/// LE-88, fixed on the bench on 2026-08-06 and never held by a test until now:
/// the Pi 5 firmware fetches `config.txt` bare at one boot stage and
/// `/config.txt` at a later one, and `Path.Combine(root, "/config.txt")`
/// DISCARDS the root. The traversal guard then correctly refused a path that
/// should never have reached it, `pciex4_reset=0` never applied, the firmware
/// reset the RP1 PCIe link, and the board reported confession code 2. The
/// kernel diagnosed the host tool's bug precisely and a human went looking at
/// the kernel.
///
/// Both directions are asserted here, per ADR 0005: the legitimate spelling is
/// served, and the escape is still refused. A fix verified in only one
/// direction is a guard that may have been deleted.
public sealed class TftpPathResolutionTests
{
    private static readonly string Root = Path.GetFullPath(Path.Combine(Path.GetTempPath(), "tos64-netboot-tests-root"));

    [Theory]
    [InlineData("config.txt")]                      // the early boot stage
    [InlineData("/config.txt")]                     // the later one: LE-88
    [InlineData("\\config.txt")]                    // the same thing, spelled Windows-ways
    [InlineData("//config.txt")]                    // and doubled, which a client may do
    public void Every_spelling_of_a_root_file_resolves_to_that_file(string requested)
    {
        var resolved = TftpPaths.Resolve(Root, requested);
        Assert.Equal(Path.Combine(Root, "config.txt"), resolved);
    }

    [Fact]
    public void A_subdirectory_is_reachable_because_the_firmware_uses_one()
    {
        var resolved = TftpPaths.Resolve(Root, "/overlays/rp1.dtbo");
        Assert.Equal(Path.Combine(Root, "overlays", "rp1.dtbo"), resolved);
    }

    [Theory]
    [InlineData("../../../secrets.txt")]
    [InlineData("/../secrets.txt")]
    [InlineData("subdir/../../secrets.txt")]
    [InlineData("C:\\Windows\\System32\\config\\SAM")]
    public void An_escape_is_still_refused(string requested)
    {
        Assert.Null(TftpPaths.Resolve(Root, requested));
    }

    /// The root's own sibling is not the root. `…-root2` starts with the same
    /// characters as `…-root`, and a `StartsWith` guard with no separator check
    /// admits it.
    [Fact]
    public void A_sibling_directory_sharing_the_root_prefix_is_refused()
    {
        Assert.Null(TftpPaths.Resolve(Root, "../tos64-netboot-tests-root2/kernel8.img"));
    }

    [Fact]
    public void An_empty_name_resolves_to_nothing_rather_than_to_the_root_itself()
    {
        Assert.Null(TftpPaths.Resolve(Root, ""));
        Assert.Null(TftpPaths.Resolve(Root, "/"));
    }
}

/// LE-87's second part: log the served file's sha256 on every transfer, so the
/// operator can compare what was served against what was built. The run that
/// failed on 2026-08-06 -- a stale kernel8.img served by a stale process --
/// would have been diagnosed in one line.
public sealed class TransferLogTests
{
    private static readonly byte[] Payload = Encoding.ASCII.GetBytes("kernel8.img stand-in");

    [Fact]
    public void The_served_line_carries_the_full_digest_of_the_bytes_that_went_out()
    {
        var expected = Convert.ToHexStringLower(SHA256.HashData(Payload));

        var line = TransferLog.Served(@"C:\Code\TinyOS\os\target\pi5\kernel8.img", Payload);

        Assert.Contains(expected, line);
        // Truncated digests compare equal for far too many pairs of images to
        // be worth the shorter line. The whole point is telling two builds of
        // the same file apart.
        Assert.Equal(64, expected.Length);
    }

    /// The absolute path names the root, which is the half LE-87 needed: the
    /// stale instance was serving a DIFFERENT root, and its own log said so in
    /// a window nobody was looking at.
    [Fact]
    public void The_served_line_names_the_absolute_path_and_the_byte_count()
    {
        var line = TransferLog.Served(@"C:\Code\TinyOS\os\target\pi5\kernel8.img", Payload);

        Assert.Contains(@"C:\Code\TinyOS\os\target\pi5\kernel8.img", line);
        Assert.Contains(Payload.Length.ToString(), line);
    }
}
