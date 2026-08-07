// The prompt: the first interactive terminal this OS has ever had
// (STORY-P1-09-17 / hand-2026-08-07/15A Part IV).
//
// A person types PING. A frame leaves this laptop. The board classifies it
// through a deny-by-default table, answers on the cable, and the answer prints
// here. That exchange is the whole point of the file, and everything in it is
// arranged so the exchange can be tested with no board, no cable and no Npcap.
//
// Three rules carried over from the rest of this tool, because a console that
// smooths anything is worse than no console:
//
//   1. A timeout prints as a TIMEOUT, never as a refusal. They mean opposite
//      things — a refusal is the board declining, a timeout is the board not
//      heard from — and a console that showed one as the other would send an
//      operator looking for a bug in the verb table when the cable is out.
//   2. A refusal prints the name the WIRE gave it. Not a local re-wording, not
//      a guess: the board owns the refusal vocabulary and this end repeats it.
//   3. An unknown verb is SENT, not pre-refused. If this tool refused locally,
//      the operator would never see the board deny by default — which is the
//      property STORY-P1-09-17 is answerable for and the thing worth watching.
//
// The seams are here for that last reason too: `IConsoleLink` is the whole of
// this file's contact with the wire, so ConsoleTests drives a scripted session
// — prompt in, answer out, refusal named, timeout named — on a bench with
// nothing plugged in.

using System.Text;

/// Whatever carries frames to the board and back. One method each way, so a
/// scripted double is four lines and the real one is a pcap handle.
internal interface IConsoleLink
{
    void Send(byte[] frame);

    /// The next TOS64 payload (Ethernet header already stripped), or null when
    /// `timeout` passes with nothing heard.
    byte[]? NextPayload(TimeSpan timeout);
}

/// The `TOS64-CMD/1` envelope and the `TOS64-ANS/1` answer, as this end of the
/// cable sees them.
///
/// Every constant here mirrors os/src/hal-arm64/src/tos64_cmd.rs, and
/// ConsoleParityTests reads that file and fails if the two ever disagree —
/// LE-80's lesson applied before the drift rather than after it.
internal static class Cmd
{
    /// hal_arm64::gem::BEACON_SOURCE_MAC.
    internal static readonly byte[] BoardMac = [0x02, 0x54, 0x4F, 0x53, 0x36, 0x34];

    /// This tool's source address (see Send.cs — the board is proven
    /// indifferent to it, so a constant makes frames reproducible).
    internal static readonly byte[] SenderMac = [0x02, 0x54, 0x44, 0x49, 0x4E, 0x4B];

    /// hal_arm64::gem::BEACON_ETHERTYPE.
    internal const ushort EtherType = 0x88B5;

    /// hal_arm64::gem_receive::ENVELOPE_PREFIX.
    internal const string EnvelopePrefix = "TOS64-";

    /// hal_arm64::tos64_cmd::COMMAND_MAGIC.
    internal const string Magic = "CMD1";

    /// hal_arm64::tos64_cmd::COMMAND_PAYLOAD_BYTES. The whole frame is
    /// 14 + 46 = 60 octets, exactly the Ethernet minimum — which is why no
    /// NIC's padding can ever reach the board's fixed-width classifier.
    internal const int PayloadBytes = 46;

    internal const int MagicOffset = 6;
    internal const int VerbOffset = 10;
    internal const int SequenceOffset = 12;
    internal const int ArgumentOffset = 16;

    /// hal_arm64::tos64_cmd::VERB_TABLE. Two rows, and this end may not
    /// invent a third: an id absent from the board's table is refused there,
    /// which is the behaviour worth showing an operator.
    internal static readonly (string Name, ushort Id)[] Verbs = [("PING", 1), ("STATUS", 2)];

    /// hal_arm64::tos64_cmd::CommandRefusal — the wire names, in order.
    internal static readonly string[] Refusals =
        ["wrong-magic", "undersize", "oversize", "unknown-verb", "over-rate"];

    /// The id an unrecognised word is sent as. Zero is not a verb on the
    /// board (no row may hold it), so it is guaranteed to come back as
    /// `unknown-verb` — a deliberate demonstration of deny-by-default rather
    /// than a local refusal that would hide it.
    internal const ushort UnknownVerbId = 0;

    internal static ushort IdFor(string word)
    {
        foreach (var (name, id) in Verbs)
        {
            if (string.Equals(name, word, StringComparison.OrdinalIgnoreCase)) return id;
        }
        return UnknownVerbId;
    }

    /// Builds one command frame: header, envelope tag, magic, verb, sequence,
    /// and a zero argument field. Exactly 60 octets, always.
    internal static byte[] Frame(ushort verbId, uint sequence)
    {
        var frame = new byte[14 + PayloadBytes];
        Array.Copy(BoardMac, 0, frame, 0, 6);
        Array.Copy(SenderMac, 0, frame, 6, 6);
        frame[12] = EtherType >> 8;
        frame[13] = EtherType & 0xFF;
        var payload = 14;
        Encoding.ASCII.GetBytes(EnvelopePrefix).CopyTo(frame, payload);
        Encoding.ASCII.GetBytes(Magic).CopyTo(frame, payload + MagicOffset);
        frame[payload + VerbOffset] = (byte)(verbId >> 8);
        frame[payload + VerbOffset + 1] = (byte)verbId;
        frame[payload + SequenceOffset] = (byte)(sequence >> 24);
        frame[payload + SequenceOffset + 1] = (byte)(sequence >> 16);
        frame[payload + SequenceOffset + 2] = (byte)(sequence >> 8);
        frame[payload + SequenceOffset + 3] = (byte)sequence;
        return frame;
    }

    /// One line the board spoke back.
    ///
    /// `Refused` and `Verb` are mutually exclusive by construction: the board
    /// either answered a row or named a refusal, and a reader that had to
    /// guess which would be exactly the ambiguity rule 1 above forbids.
    internal sealed record Answer(string? Verb, uint Sequence, string? Refused, uint Dropped, string? Status);

    /// Parses a `TOS64-ANS/1` line, or null when the line is not one.
    ///
    /// Deliberately strict about the prefix and tolerant about field order:
    /// the board's renderer pins the order with its own tests, and a host
    /// parser that also pinned it would fail a capture for a cosmetic reason.
    internal static Answer? Parse(string line)
    {
        const string Sentinel = "TOS64-ANS/1 ";
        if (!line.StartsWith(Sentinel, StringComparison.Ordinal)) return null;

        string? verb = null, refused = null, status = null;
        uint sequence = 0, dropped = 0;
        var rest = line[Sentinel.Length..].TrimEnd();
        // `status=` runs to the end of the line by construction (it carries a
        // whole verdict line, spaces included), so it is split off first.
        var statusAt = rest.IndexOf("status=", StringComparison.Ordinal);
        if (statusAt >= 0)
        {
            status = rest[(statusAt + "status=".Length)..];
            rest = rest[..statusAt].TrimEnd();
        }
        foreach (var field in rest.Split(' ', StringSplitOptions.RemoveEmptyEntries))
        {
            var eq = field.IndexOf('=');
            if (eq <= 0) continue;
            var key = field[..eq];
            var value = field[(eq + 1)..];
            switch (key)
            {
                case "verb": verb = value; break;
                case "refused": refused = value; break;
                case "seq": uint.TryParse(value, out sequence); break;
                case "dropped": uint.TryParse(value, out dropped); break;
            }
        }
        if (verb is null && refused is null) return null;
        return new Answer(verb, sequence, refused, dropped, status);
    }

    /// Every TOS64 text line in a payload — the board zero-pads its text
    /// frames, so a run of printable ASCII terminated by NUL is one line.
    internal static IEnumerable<string> Lines(byte[] payload)
    {
        var at = 0;
        while (at < payload.Length)
        {
            if (payload[at] < 0x20 || payload[at] > 0x7E) { at++; continue; }
            var start = at;
            while (at < payload.Length && payload[at] >= 0x20 && payload[at] <= 0x7E) at++;
            yield return Encoding.ASCII.GetString(payload, start, at - start).TrimEnd();
        }
    }
}

internal static class ConsoleMode
{
    /// The prompt printed before each line read.
    internal const string Prompt = "tos64> ";

    /// How long one exchange waits for the board before it says so.
    ///
    /// The board answers at most one line per park beat and the beat is 1 Hz,
    /// so a single beat is the floor and anything under it would time out on a
    /// board that is working perfectly. Four is the floor with room for the
    /// beat the command arrived in and the one it is answered in — a UX bound,
    /// not a measured one, and stated as such because this project does not
    /// quote timing it has not measured.
    internal const int DefaultTimeoutSeconds = 4;

    /// The interactive loop, driven entirely over its arguments so a scripted
    /// session runs green with no board and no Npcap.
    ///
    /// Returns the number of exchanges that produced no answer at all — the
    /// machine-readable half, so a scripted bench run can gate on it.
    internal static int Run(TextReader input, TextWriter output, IConsoleLink link, int timeoutSeconds)
    {
        output.WriteLine("Ti64Dink console — TOS64-CMD/1 (STORY-P1-09-17)");
        output.WriteLine("  verbs: " + string.Join(" | ", Cmd.Verbs.Select(v => v.Name))
            + "   (anything else is SENT, so the board's refusal is the one you see)");
        output.WriteLine("  quit  : `quit`, `exit`, or end of input");
        output.WriteLine();

        var timeout = TimeSpan.FromSeconds(timeoutSeconds);
        uint sequence = 1;
        var silent = 0;
        while (true)
        {
            output.Write(Prompt);
            output.Flush();
            var typed = input.ReadLine();
            if (typed is null) break;
            var word = typed.Trim();
            if (word.Length == 0) continue;
            if (word is "quit" or "exit") break;

            var id = Cmd.IdFor(word);
            link.Send(Cmd.Frame(id, sequence));
            output.WriteLine($"    sent   : verb={word.ToUpperInvariant()} id={id} seq={sequence}");

            var answer = Await(link, sequence, timeout);
            if (answer is null)
            {
                // Rule 1. A timeout is a timeout.
                silent++;
                output.WriteLine($"    TIMEOUT: nothing answered seq={sequence} within {timeoutSeconds}s");
                output.WriteLine("             (the board not heard from — NOT a refusal)");
            }
            else if (answer.Refused is not null)
            {
                // Rule 2. The wire's own name for it.
                output.WriteLine(answer.Refused == "over-rate"
                    ? $"    REFUSED: {answer.Refused} dropped={answer.Dropped}"
                    : $"    REFUSED: {answer.Refused} seq={answer.Sequence}");
            }
            else
            {
                output.WriteLine($"    ANSWER : verb={answer.Verb} seq={answer.Sequence}"
                    + (answer.Status is null ? "" : $" status={answer.Status}"));
            }
            sequence++;
        }
        output.WriteLine();
        output.WriteLine($"console: {silent} exchange(s) went unanswered");
        return silent;
    }

    /// Waits for the board's next answer line, ignoring the beacons and
    /// transcript rows that share this EtherType.
    ///
    /// An answer for an OLDER sequence is skipped rather than accepted: a
    /// stale line would print under the command the operator just typed and
    /// read as its answer, which is the one way this console could lie.
    private static Cmd.Answer? Await(IConsoleLink link, uint sequence, TimeSpan timeout)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (DateTime.UtcNow < deadline)
        {
            var payload = link.NextPayload(deadline - DateTime.UtcNow);
            if (payload is null) return null;
            foreach (var line in Cmd.Lines(payload))
            {
                if (Cmd.Parse(line) is not { } answer) continue;
                // Over-rate names no sequence of its own; it is always about
                // the flood that is happening now, so it is never stale.
                if (answer.Refused == "over-rate" || answer.Sequence == sequence) return answer;
            }
        }
        return null;
    }
}
