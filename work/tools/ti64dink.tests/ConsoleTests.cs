// The console, tested with nothing plugged in (STORY-P1-09-17 / 15A Part IV).
//
// The frame builder and the answer parser are pure and asserted byte for byte
// — the Send.Frame pattern. The loop itself runs over injected reader, writer
// and link seams, so a whole scripted session (prompt in, answer out, refusal
// named, timeout named) is a host test rather than a bench observation.
//
// That matters more here than for most files in this tool. The console is the
// thing an operator will be staring at while a board is powered, and the two
// failure modes that would waste that afternoon — a timeout printed as a
// refusal, a stale answer printed under a fresh command — are exactly the ones
// no board session diagnoses quickly and both of which are pinned below.

using System.Text;

public sealed class ConsoleTests
{
    /// A link with a script: what it was sent, and what it will answer.
    private sealed class ScriptedLink : IConsoleLink
    {
        internal readonly List<byte[]> Sent = [];
        private readonly Queue<byte[]?> _answers = new();

        internal ScriptedLink Answers(string line)
        {
            _answers.Enqueue(Encoding.ASCII.GetBytes(line + "\0\0"));
            return this;
        }

        /// A beat in which nothing came back.
        internal ScriptedLink Silent()
        {
            _answers.Enqueue(null);
            return this;
        }

        public void Send(byte[] frame) => Sent.Add(frame);

        public byte[]? NextPayload(TimeSpan timeout) =>
            _answers.Count > 0 ? _answers.Dequeue() : null;
    }

    private static string Session(string typed, ScriptedLink link, out int unanswered)
    {
        var output = new StringWriter();
        // One second, because every timeout arm here is a scripted absence
        // rather than a real wait — the test must not spend the UX bound.
        unanswered = ConsoleMode.Run(new StringReader(typed), output, link, 1);
        return output.ToString();
    }

    // --- the frame builder, exact bytes -------------------------------------

    [Fact]
    public void ACommandFrameIsExactlyTheEthernetMinimumAndCarriesTheFixedLayout()
    {
        var frame = Cmd.Frame(Cmd.IdFor("PING"), 7);

        Assert.Equal(60, frame.Length);
        Assert.Equal(14 + Cmd.PayloadBytes, frame.Length);
        Assert.Equal(Cmd.BoardMac, frame[0..6]);
        Assert.Equal(Cmd.SenderMac, frame[6..12]);
        Assert.Equal(0x88, frame[12]);
        Assert.Equal(0xB5, frame[13]);
        Assert.Equal("TOS64-", Encoding.ASCII.GetString(frame, 14, 6));
        Assert.Equal("CMD1", Encoding.ASCII.GetString(frame, 14 + Cmd.MagicOffset, 4));
        // Verb and sequence, big-endian, at their fixed offsets.
        Assert.Equal(new byte[] { 0x00, 0x01 }, frame[(14 + Cmd.VerbOffset)..(14 + Cmd.VerbOffset + 2)]);
        Assert.Equal(
            new byte[] { 0x00, 0x00, 0x00, 0x07 },
            frame[(14 + Cmd.SequenceOffset)..(14 + Cmd.SequenceOffset + 4)]);
        // The argument field is present, fixed-width and zero.
        Assert.All(frame[(14 + Cmd.ArgumentOffset)..], b => Assert.Equal(0, b));
    }

    [Fact]
    public void TheWholeFrameIsTheEthernetMinimumSoNoNicPaddingCanEverReachTheBoard()
    {
        // The reason the layout is 46 octets and not 32: a shorter frame is
        // padded to 60 by the NIC, below any software here, and the padding
        // would arrive at the board's fixed-width classifier indistinguishable
        // from a wrong-width field. This assertion is that argument.
        foreach (var (_, id) in Cmd.Verbs)
        {
            Assert.Equal(60, Cmd.Frame(id, uint.MaxValue).Length);
        }
        Assert.Equal(60, Cmd.Frame(Cmd.UnknownVerbId, 0).Length);
    }

    [Fact]
    public void AnUnrecognisedWordIsSentAsAnIdTheBoardCannotHoldRatherThanRefusedHere()
    {
        // Deny-by-default is the board's property and the operator has to be
        // able to WATCH it. A console that refused locally would show the same
        // text and prove nothing.
        Assert.Equal(0, Cmd.UnknownVerbId);
        Assert.Equal(Cmd.UnknownVerbId, Cmd.IdFor("banana"));
        Assert.Equal(Cmd.UnknownVerbId, Cmd.IdFor(""));
        Assert.Equal(1, Cmd.IdFor("ping"));
        Assert.Equal(1, Cmd.IdFor("PING"));
        Assert.Equal(2, Cmd.IdFor("Status"));
    }

    // --- the parser ----------------------------------------------------------

    [Fact]
    public void EveryAnswerAndRefusalShapeTheBoardCanEmitParses()
    {
        var ping = Cmd.Parse("TOS64-ANS/1 verb=PING seq=11 ok=1");
        Assert.NotNull(ping);
        Assert.Equal("PING", ping!.Verb);
        Assert.Equal(11u, ping.Sequence);
        Assert.Null(ping.Refused);
        Assert.Null(ping.Status);

        var status = Cmd.Parse("TOS64-ANS/1 verb=STATUS seq=2 ok=1 status=TOS64-RESULT/1 fixture=boot ok=true");
        Assert.NotNull(status);
        Assert.Equal("STATUS", status!.Verb);
        // The verdict line runs to the end of the line, spaces and all — a
        // parser that split it on the first space would report half a verdict.
        Assert.Equal("TOS64-RESULT/1 fixture=boot ok=true", status.Status);

        var refused = Cmd.Parse("TOS64-ANS/1 refused=unknown-verb seq=3");
        Assert.NotNull(refused);
        Assert.Equal("unknown-verb", refused!.Refused);
        Assert.Equal(3u, refused.Sequence);
        Assert.Null(refused.Verb);

        var overRate = Cmd.Parse("TOS64-ANS/1 refused=over-rate dropped=4");
        Assert.Equal("over-rate", overRate!.Refused);
        Assert.Equal(4u, overRate.Dropped);
    }

    [Fact]
    public void EverythingElseOnThisEtherTypeParsesAsNotAnAnswer()
    {
        // The board's beacon and its transcript rows share the EtherType, and
        // a console that mistook one for an answer would print the board's own
        // chatter as a reply to what the operator typed.
        Assert.Null(Cmd.Parse("TOS64-PRESENT/1 seq=5964"));
        Assert.Null(Cmd.Parse("TOS64-RX/1 state=listening accepted=1 refused=0"));
        Assert.Null(Cmd.Parse(""));
        Assert.Null(Cmd.Parse("TOS64-ANS/1 "));
    }

    // --- the loop ------------------------------------------------------------

    [Fact]
    public void AScriptedSessionAnswersRefusesAndTimesOutInTheOperatorSOwnOrder()
    {
        var link = new ScriptedLink()
            .Answers("TOS64-ANS/1 verb=PING seq=1 ok=1")
            .Answers("TOS64-ANS/1 verb=STATUS seq=2 ok=1 status=TOS64-RESULT/1 fixture=boot ok=true")
            .Answers("TOS64-ANS/1 refused=unknown-verb seq=3")
            .Silent();

        var transcript = Session("PING\nSTATUS\nbanana\nPING\nquit\n", link, out var unanswered);

        Assert.Equal(4, link.Sent.Count);
        Assert.Contains("ANSWER : verb=PING seq=1", transcript);
        Assert.Contains("ANSWER : verb=STATUS seq=2 status=TOS64-RESULT/1 fixture=boot ok=true", transcript);
        Assert.Contains("REFUSED: unknown-verb seq=3", transcript);
        Assert.Contains("TIMEOUT: nothing answered seq=4", transcript);
        Assert.Equal(1, unanswered);
    }

    [Fact]
    public void ATimeoutIsNeverPrintedAsARefusalAndARefusalIsNeverPrintedAsATimeout()
    {
        // The two mean opposite things — the board declining, versus the board
        // not heard from — and telling them apart is the difference between an
        // operator checking the verb table and an operator checking the cable.
        var silent = Session("PING\n", new ScriptedLink().Silent(), out var unanswered);
        Assert.Contains("TIMEOUT", silent);
        Assert.DoesNotContain("REFUSED", silent);
        Assert.Equal(1, unanswered);

        var refused = Session("PING\n",
            new ScriptedLink().Answers("TOS64-ANS/1 refused=wrong-magic seq=1"), out unanswered);
        Assert.Contains("REFUSED: wrong-magic", refused);
        Assert.DoesNotContain("TIMEOUT", refused);
        Assert.Equal(0, unanswered);
    }

    [Fact]
    public void ARefusalPrintsTheNameTheWireGaveItAndNothingIsSmoothed()
    {
        foreach (var name in Cmd.Refusals)
        {
            var line = name == "over-rate"
                ? "TOS64-ANS/1 refused=over-rate dropped=2"
                : $"TOS64-ANS/1 refused={name} seq=1";
            var transcript = Session("PING\n", new ScriptedLink().Answers(line), out _);
            Assert.Contains($"REFUSED: {name}", transcript);
        }
    }

    [Fact]
    public void AStaleAnswerIsNeverPrintedUnderTheCommandTheOperatorJustTyped()
    {
        // The board answers one line per beat and a capture window holds
        // whatever was in flight. An answer for seq 1 arriving while seq 2 is
        // outstanding must be skipped, not shown — otherwise the console
        // reports an old success as a new one, which is the only way this file
        // could lie to somebody standing at a bench.
        var link = new ScriptedLink()
            .Answers("TOS64-ANS/1 verb=PING seq=1 ok=1")   // answers command 1
            .Answers("TOS64-ANS/1 verb=PING seq=1 ok=1")   // stale, arrives during command 2
            .Silent();
        var transcript = Session("PING\nPING\n", link, out var unanswered);

        Assert.Contains("ANSWER : verb=PING seq=1", transcript);
        Assert.Contains("TIMEOUT: nothing answered seq=2", transcript);
        Assert.Equal(1, unanswered);
    }

    [Fact]
    public void TheBoardsBeaconsAndTranscriptRowsAreIgnoredWhileWaitingForAnAnswer()
    {
        var link = new ScriptedLink()
            .Answers("TOS64-PRESENT/1 seq=5964")
            .Answers("TOS64-MEAS/2 BEGIN tier=T1")
            .Answers("TOS64-ANS/1 verb=PING seq=1 ok=1");
        var transcript = Session("PING\n", link, out var unanswered);
        Assert.Contains("ANSWER : verb=PING seq=1", transcript);
        Assert.Equal(0, unanswered);
    }

    [Fact]
    public void EndOfInputEndsTheSessionAsCleanlyAsQuitDoes()
    {
        foreach (var typed in new[] { "", "quit\n", "exit\n" })
        {
            var link = new ScriptedLink();
            var transcript = Session(typed, link, out var unanswered);
            Assert.Empty(link.Sent);
            Assert.Equal(0, unanswered);
            Assert.Contains("0 exchange(s) went unanswered", transcript);
        }
    }

    [Fact]
    public void TheSequenceAdvancesOnceForEveryCommandSentAndNeverForAnythingElse()
    {
        var link = new ScriptedLink();
        Session("PING\n\n   \nPING\n", link, out _);
        Assert.Equal(2, link.Sent.Count);
        // Blank lines are not commands: sequence 1 then 2, no gap.
        Assert.Equal(1, link.Sent[0][14 + Cmd.SequenceOffset + 3]);
        Assert.Equal(2, link.Sent[1][14 + Cmd.SequenceOffset + 3]);
    }
}
