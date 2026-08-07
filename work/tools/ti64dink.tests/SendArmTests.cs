// STORY-P1-09-16 criterion 4's sender, under test (10A §3 S3: "put the
// console/--send paths under test" before M4 makes this tool the interface).
//
// The arms' whole value is that each is wrong in exactly one field — a builder
// that drifted, padded differently, or "fixed up" a deliberately-wrong byte
// would make a refusal arm untestable on the bench while looking healthy in
// its own prose. The Rust half (`gem_receive::admit` asserted against
// `--send-frames` output) guards the verdicts; this half guards the frames.

using System.Text;

public sealed class SendArmTests
{
    /// hal_arm64::gem::BEACON_SOURCE_MAC — stated here independently rather
    /// than read from the code under test, so a drifted constant fails.
    private static readonly byte[] BoardMac = [0x02, 0x54, 0x4F, 0x53, 0x36, 0x34];

    private static readonly byte[] Broadcast = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

    /// Locally administered, ASCII `TDINK` behind the local bit.
    private static readonly byte[] SenderMac = [0x02, 0x54, 0x44, 0x49, 0x4E, 0x4B];

    private static ushort EtherTypeOf(byte[] frame) => (ushort)((frame[12] << 8) | frame[13]);

    [Fact]
    public void every_arm_pads_to_the_ethernet_minimum_and_names_our_sender()
    {
        foreach (var arm in Send.Arms)
        {
            var frame = Send.Frame(arm);
            Assert.True(frame.Length >= 60, $"{arm.Name}: {frame.Length} octets is under the minimum");
            Assert.Equal(SenderMac, frame[6..12]);
            var payload = Encoding.ASCII.GetBytes(arm.Payload);
            Assert.Equal(payload, frame[14..(14 + payload.Length)]);
            // The padding is zeros, not garbage: what leaves the tool is what
            // the operator was told would leave it.
            for (var i = 14 + payload.Length; i < frame.Length; i++)
            {
                Assert.Equal(0, frame[i]);
            }
        }
    }

    [Fact]
    public void the_accept_arms_differ_only_in_their_destination()
    {
        var ping = Send.Frame(Send.Find("ping")!);
        var unicast = Send.Frame(Send.Find("unicast")!);
        Assert.Equal(Broadcast, ping[..6]);
        Assert.Equal(BoardMac, unicast[..6]);
        Assert.Equal((ushort)0x88B5, EtherTypeOf(ping));
        Assert.Equal((ushort)0x88B5, EtherTypeOf(unicast));
        Assert.StartsWith("TOS64-", Send.Find("ping")!.Payload, StringComparison.Ordinal);
        Assert.StartsWith("TOS64-", Send.Find("unicast")!.Payload, StringComparison.Ordinal);
    }

    [Fact]
    public void each_refusal_arm_is_wrong_in_exactly_one_field()
    {
        // ethertype: IPv4 where 0x88B5 belongs; everything else a valid envelope.
        var ethertype = Send.Find("ethertype")!;
        Assert.Equal((ushort)0x0800, EtherTypeOf(Send.Frame(ethertype)));
        Assert.Equal(Broadcast, Send.Frame(ethertype)[..6]);
        Assert.StartsWith("TOS64-", ethertype.Payload, StringComparison.Ordinal);

        // prefix: the retired protocol name where TOS64- belongs. This is the
        // one place TINYOS- may appear in new work — as the wrong bytes a
        // refusal arm exists to send.
        var prefix = Send.Find("prefix")!;
        Assert.Equal((ushort)0x88B5, EtherTypeOf(Send.Frame(prefix)));
        Assert.StartsWith("TINYOS-", prefix.Payload, StringComparison.Ordinal);

        // notforus: a stranger's address; EtherType and payload both valid, so
        // only the hardware filter can be what drops it.
        var notforus = Send.Find("notforus")!;
        var frame = Send.Frame(notforus);
        Assert.NotEqual(Broadcast, frame[..6]);
        Assert.NotEqual(BoardMac, frame[..6]);
        Assert.Equal((ushort)0x88B5, EtherTypeOf(frame));
        Assert.StartsWith("TOS64-", notforus.Payload, StringComparison.Ordinal);
    }

    [Fact]
    public void find_is_exact_and_refuses_unknown_arms()
    {
        Assert.NotNull(Send.Find("ping"));
        Assert.Null(Send.Find("PING"));
        Assert.Null(Send.Find("pings"));
    }

    [Fact]
    public void the_emitted_arm_file_round_trips_to_the_frames_it_describes()
    {
        var path = Path.Combine(Path.GetTempPath(), $"ti64dink-arms-{Guid.NewGuid():N}.txt");
        try
        {
            Send.Emit(path);
            var rows = File.ReadAllLines(path)
                .Where(line => !line.StartsWith('#'))
                .Select(line => line.Split(' ', 3))
                .ToList();
            Assert.Equal(Send.Arms.Length, rows.Count);
            foreach (var row in rows)
            {
                var arm = Send.Find(row[0]);
                Assert.NotNull(arm);
                Assert.Equal(arm!.Verdict, row[1]);
                // The hex IS the frame: the Rust host test parses these bytes
                // and asserts admit()'s verdict, so a drift between this file
                // and Frame() would silently test different bytes than fly.
                Assert.Equal(Convert.ToHexString(Send.Frame(arm)), row[2]);
            }
        }
        finally
        {
            File.Delete(path);
        }
    }
}
