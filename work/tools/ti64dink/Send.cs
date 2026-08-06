// The sending half of the cable (STORY-P1-09-16 criterion 4, LE-93).
//
// The board learned to receive on 2026-08-06 and nothing on this bench could
// speak to it, so its first inbound path in the project's history was
// host-Green and unwitnessable. This file is the forty lines that fix that.
//
// It is organised as a table of ARMS rather than as one --send flag, because
// criterion 4 is explicitly not satisfied by making the board hear something:
//
//     "an accepted count proves only that the board can hear, and the
//      declining is what this Story is answerable for"
//
// So each arm states, in the table, exactly what the canvas TOS64-RX/1 row is
// expected to do. The operator reads the prediction before sending and the
// canvas after, and a disagreement is the finding.
//
// Two of the board's refusals are NOT reachable from here, and they are listed
// in the table as unreachable rather than quietly omitted:
//
//   * TooShort — Ethernet pads every frame to 60 octets in the NIC, below any
//     software this tool can reach, so a frame shorter than the board's 20-byte
//     admission floor cannot be put on a wire at all. It is a host-test-only
//     refusal, and that is a property of Ethernet, not a gap in this tool.
//   * The three descriptor refusals (fragment, zero length, over length) are
//     statements about what the GEM writes into the ring. No frame a host can
//     send produces them; they are the arms that exist because the device is
//     not trusted, and only a lying device can reach them.
//
// `notforus` is the subtlest arm and the one worth reading twice: it expects
// BOTH counters to stay still. The board's hardware address filter drops the
// frame before DMA, so a moved `refused` count would mean the filter is not
// doing the job the containment argument in STORY-P1-09-16 assigns it.

using System.Text;

internal static class Send
{
    /// hal_arm64::gem::BEACON_SOURCE_MAC — the board's own address, and what
    /// STORY-P1-09-16 programs into the GEM's SA1B/SA1T filter.
    private static readonly byte[] BoardMac = [0x02, 0x54, 0x4F, 0x53, 0x36, 0x34];

    /// This tool's source address: locally administered, ASCII `TDINK` behind
    /// the local bit. A constant rather than the adapter's real MAC because the
    /// board is proven indifferent to the source field
    /// (`gem_receive::tests::admission_is_indifferent_to_every_byte_it_does_not_name`),
    /// so a constant makes the frame reproducible instead of bench-specific.
    private static readonly byte[] SenderMac = [0x02, 0x54, 0x44, 0x49, 0x4E, 0x4B];

    /// An address that is neither broadcast nor the board's, for the arm that
    /// tests the hardware filter.
    private static readonly byte[] StrangerMac = [0x02, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA];

    private static readonly byte[] Broadcast = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

    /// hal_arm64::gem::BEACON_ETHERTYPE.
    private const ushort Tos64EtherType = 0x88B5;

    /// IPv4 — an EtherType the board must decline, chosen because it is the
    /// one most likely to arrive by accident on a shared segment.
    private const ushort Ipv4EtherType = 0x0800;

    /// Ethernet's minimum frame length without FCS. Everything is padded to it
    /// here rather than left to the NIC, so what leaves this tool is what the
    /// operator was told would leave it.
    private const int MinimumFrameLen = 60;

    internal sealed record Arm(
        string Name,
        string What,
        string Expect,
        byte[] Destination,
        ushort EtherType,
        string Payload,
        string Verdict);

    internal static readonly Arm[] Arms =
    [
        new("ping",
            "broadcast, 0x88B5, payload begins TOS64-",
            "accepted +1, refused unchanged",
            Broadcast, Tos64EtherType, "TOS64-PING/1 from=ti64dink\n",
            "Accepted"),

        new("unicast",
            "addressed to the board's own MAC, otherwise identical to `ping`",
            "accepted +1 — proves the filter admits our address, not only broadcast",
            BoardMac, Tos64EtherType, "TOS64-PING/1 from=ti64dink addressed=1\n",
            "Accepted"),

        new("ethertype",
            "broadcast, EtherType 0x0800 (IPv4), payload begins TOS64-",
            "refused +1, accepted unchanged — WrongEtherType",
            Broadcast, Ipv4EtherType, "TOS64-PING/1 from=ti64dink\n",
            "WrongEtherType"),

        new("prefix",
            "broadcast, 0x88B5, payload begins TINYOS- instead",
            "refused +1, accepted unchanged — NotAnEnvelope",
            Broadcast, Tos64EtherType, "TINYOS-PING/1 from=ti64dink\n",
            "NotAnEnvelope"),

        new("notforus",
            "unicast to 02:aa:aa:aa:aa:aa, 0x88B5, payload begins TOS64-",
            "BOTH counters unchanged — the hardware address filter drops it before DMA; "
                + "a moved `refused` means the filter is not containing what it is supposed to",
            StrangerMac, Tos64EtherType, "TOS64-PING/1 from=ti64dink stranger=1\n",
            "NotAddressedHere"),
    ];

    /// Writes every arm as `<name> <verdict> <hex>`, for the Rust host test
    /// that asserts `gem_receive::admit` returns exactly the verdict this table
    /// predicts — for every arm, before any board is powered.
    ///
    /// This is the `LE-80` mirror shape and the reason it is worth the file: a
    /// sender whose expectations are written in its own prose is a sender whose
    /// expectations can drift from the filter they describe, and the drift
    /// would only ever surface as a confusing board session. Derived from the
    /// table rather than kept beside it, the two cannot disagree without a test
    /// going red on a laptop.
    ///
    /// The `notforus` arm is the one to read carefully: its recorded verdict is
    /// the SOFTWARE one (`NotAddressedHere`), and on real hardware the frame
    /// should never reach the software at all because the GEM's address filter
    /// drops it first. The test asserts the software half; the board session
    /// asserts the hardware half by BOTH counters staying still.
    internal static void Emit(string path)
    {
        var lines = new List<string>
        {
            "# STORY-P1-09-16 criterion 4: the frames ti64dink --send transmits, and the",
            "# verdict gem_receive::admit must return for each. Format: <arm> <verdict> <hex>.",
            "# Generated by `ti64dink --send-frames <path>`; asserted by a Rust host test.",
            "#",
            "# `notforus` records the SOFTWARE verdict. On hardware the GEM address filter",
            "# should drop that frame before DMA, so the board-side expectation is that",
            "# NEITHER counter moves — which no host test can check and only a board can.",
        };
        foreach (var arm in Arms)
        {
            lines.Add($"{arm.Name} {arm.Verdict} {Convert.ToHexString(Frame(arm))}");
        }
        File.WriteAllLines(path, lines);
        Console.WriteLine($"ti64dink: {Arms.Length} arm frame(s) written to {path}");
    }

    internal static Arm? Find(string name)
    {
        foreach (var arm in Arms)
        {
            if (string.Equals(arm.Name, name, StringComparison.Ordinal)) return arm;
        }
        return null;
    }

    /// Builds one arm's frame, verbatim and padded to the Ethernet minimum.
    ///
    /// Nothing here validates or corrects: the refusal arms are frames that are
    /// deliberately wrong in exactly one field, and a builder that fixed them
    /// up would make every refusal untestable.
    internal static byte[] Frame(Arm arm)
    {
        var payload = Encoding.ASCII.GetBytes(arm.Payload);
        var length = Math.Max(14 + payload.Length, MinimumFrameLen);
        var frame = new byte[length];
        Array.Copy(arm.Destination, 0, frame, 0, 6);
        Array.Copy(SenderMac, 0, frame, 6, 6);
        frame[12] = (byte)(arm.EtherType >> 8);
        frame[13] = (byte)arm.EtherType;
        Array.Copy(payload, 0, frame, 14, payload.Length);
        return frame;
    }

    internal static void Describe()
    {
        Console.WriteLine("Arms (STORY-P1-09-16 criterion 4 — the accepted AND the refused half):");
        Console.WriteLine();
        foreach (var arm in Arms)
        {
            Console.WriteLine($"  {arm.Name}");
            Console.WriteLine($"      sends  : {arm.What}");
            Console.WriteLine($"      expect : {arm.Expect}");
        }
        Console.WriteLine();
        Console.WriteLine("Not reachable from a host, and listed so their absence is not read as a gap:");
        Console.WriteLine("  TooShort              — the NIC pads every frame to 60 octets below any");
        Console.WriteLine("                          software here, so a frame under the board's 20-byte");
        Console.WriteLine("                          admission floor cannot be put on a wire. Host-test only.");
        Console.WriteLine("  fragment / zero-length / over-length");
        Console.WriteLine("                        — statements about what the GEM writes into the ring.");
        Console.WriteLine("                          Only a lying device reaches them, never a peer.");
    }
}
