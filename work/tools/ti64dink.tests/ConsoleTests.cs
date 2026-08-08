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
    public void ACommandFrameIsNeverPaddableAndCarriesTheFixedLayout()
    {
        var frame = Cmd.Frame(Cmd.IdFor("PING"), 7);

        Assert.Equal(158, frame.Length);
        Assert.True(frame.Length >= 60, "at or above the Ethernet minimum, so no NIC pads it");
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
    public void EveryFrameIsAtOrAboveTheEthernetMinimumSoNoNicPaddingCanEverReachTheBoard()
    {
        // A frame UNDER 60 octets is padded to 60 by the NIC, below any
        // software here, and the padding would arrive at the board's
        // fixed-width classifier indistinguishable from a wrong-width field.
        // The property is therefore `>= 60`, not `== 60` — a NIC never pads a
        // frame already at or above the minimum. Pinning `== 60` is what held
        // the command line at 30 octets until 2026-08-08 for a guarantee that
        // `>=` gives at any width.
        const int EthernetMinimum = 60;
        foreach (var (_, id) in Cmd.Verbs)
        {
            Assert.True(Cmd.Frame(id, uint.MaxValue).Length >= EthernetMinimum);
        }
        Assert.True(Cmd.Frame(Cmd.UnknownVerbId, 0).Length >= EthernetMinimum);
        // And every frame is the SAME width, which is what makes the board's
        // classifier total: unpaddable is necessary, fixed-width is the point.
        Assert.Equal(14 + Cmd.PayloadBytes, Cmd.Frame(Cmd.IdFor("SHELL"), 1).Length);
        Assert.Equal(
            14 + Cmd.PayloadBytes,
            Cmd.Frame(Cmd.IdFor("SHELL"), 1, new string('A', Cmd.ArgumentBytes * 2)).Length);
    }

    [Fact]
    public void TheArgumentFieldCarriesTheLineTheBoardsShellAccepts()
    {
        // shell::capacities::MAX_LINE, held equal to tos64_cmd::ARGUMENT_BYTES
        // by a compile-time assertion in pi5-image. This end cannot see either
        // crate, so it pins the number and names where the agreement lives.
        Assert.Equal(128, Cmd.ArgumentBytes);

        // The command the old 30-octet field could not carry.
        const string line = "FIND /N \"Ethernet cable\" README.TXT";
        Assert.True(line.Length > 30);
        var frame = Cmd.Frame(Cmd.IdFor("SHELL"), 3, line);
        Assert.Equal(
            line,
            Encoding.ASCII.GetString(frame, 14 + Cmd.ArgumentOffset, line.Length));
        Assert.All(
            frame[(14 + Cmd.ArgumentOffset + line.Length)..],
            b => Assert.Equal(0, b));
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

    // --- the SHELL row: a human typing at TinyOS (STORY-P1-09-18) -----------

    [Fact]
    public void AShellCommandCarriesItsLineInTheFixedArgumentFieldAndNowhereElse()
    {
        var frame = Cmd.Frame(Cmd.IdFor("SHELL"), 4, "DIR A:\\");

        // Still exactly the Ethernet minimum. A command line does not grow the
        // frame; that is the whole reason the field is fixed.
        Assert.Equal(158, frame.Length);
        Assert.True(frame.Length >= 60, "at or above the Ethernet minimum, so no NIC pads it");
        Assert.Equal(new byte[] { 0x00, 0x03 }, frame[(14 + Cmd.VerbOffset)..(14 + Cmd.VerbOffset + 2)]);
        var field = frame[(14 + Cmd.ArgumentOffset)..];
        Assert.Equal(Cmd.ArgumentBytes, field.Length);
        Assert.Equal("DIR A:\\", Encoding.ASCII.GetString(field, 0, 7));
        // Everything past the line is zero — the padding the board trims.
        Assert.All(field[7..], b => Assert.Equal(0, b));
    }

    [Fact]
    public void ALineLongerThanTheFieldIsTruncatedAndTheOperatorIsToldByTheToolThatDidIt()
    {
        var link = new ScriptedLink().Answers("TOS64-ANS/1 verb=SHELL seq=1 ok=1 out=none");
        var typed = "SHELL " + new string('X', Cmd.ArgumentBytes + 5);
        var transcript = Session(typed + "\n", link, out _);

        Assert.Contains($"the command line is {Cmd.ArgumentBytes + 5} octets", transcript);
        Assert.Contains($"a frame carries {Cmd.ArgumentBytes}", transcript);
        // And the frame really is the fixed width, carrying the prefix only.
        Assert.Equal(14 + Cmd.PayloadBytes, link.Sent[0].Length);
        Assert.Equal(
            new string('X', Cmd.ArgumentBytes),
            Encoding.ASCII.GetString(link.Sent[0], 14 + Cmd.ArgumentOffset, Cmd.ArgumentBytes));
    }

    [Fact]
    public void TheBoardsShellOutputIsUnescapedBackIntoTheLinesTinycmdWrote()
    {
        var link = new ScriptedLink().Answers(
            "TOS64-ANS/1 verb=SHELL seq=1 ok=1 out= Volume in drive A is TINYOS\\nREADME.TXT\\n");
        var transcript = Session("SHELL DIR\n", link, out var unanswered);

        Assert.Equal(0, unanswered);
        Assert.Contains("verb=SHELL seq=1", transcript);
        Assert.Contains("    |  Volume in drive A is TINYOS", transcript);
        Assert.Contains("    | README.TXT", transcript);
        // The escape is gone and no literal `\n` survived into the display.
        Assert.DoesNotContain("\\n", transcript);
    }

    [Fact]
    public void OutputThatDidNotFitTheFrameIsStatedRatherThanQuietlyAbsent()
    {
        var link = new ScriptedLink().Answers(
            "TOS64-ANS/1 verb=SHELL seq=1 ok=1 out=README.TXT more=42");
        var transcript = Session("SHELL DIR\n", link, out _);

        Assert.Contains("    | README.TXT", transcript);
        Assert.Contains("42 octet(s) did not fit", transcript);
        // The count must not be mistaken for output.
        Assert.DoesNotContain("| 42", transcript);
    }

    [Fact]
    public void AnEscapedBackslashComesBackAsOneBackslashAndNotAsAnEscape()
    {
        // The path separator is the character most likely to appear in real
        // output and is also the escape character, so the round trip through
        // `A:\>` is the case worth pinning by name.
        Assert.Equal("A:\\>", Cmd.Unescape("A:\\\\>"));
        Assert.Equal("a\nb", Cmd.Unescape("a\\nb"));
        Assert.Equal("a\\nb", Cmd.Unescape("a\\\\nb"));
        // A `?` is what the board substituted for something unprintable. It is
        // shown as a `?`: inventing the octet back would be this end
        // fabricating board output.
        Assert.Equal("EVIL?[2J", Cmd.Unescape("EVIL?[2J"));
    }

    [Fact]
    public void AnEmptyOutputFieldIsAnAnswerAndNotASilence()
    {
        var link = new ScriptedLink().Answers("TOS64-ANS/1 verb=SHELL seq=1 ok=1 out=none");
        var transcript = Session("SHELL\n", link, out var unanswered);

        Assert.Equal(0, unanswered);
        Assert.Contains("verb=SHELL", transcript);
        Assert.DoesNotContain("TIMEOUT", transcript);
    }

    [Fact]
    public void AnUnknownWordIsStillSentWholeSoTheBoardIsTheOneThatDeniesIt()
    {
        // Rule 3, re-asserted now that a row exists which could have swallowed
        // it. A console that quietly routed `WOBBLE` to the shell would hide
        // the deny-by-default table an operator is meant to be watching.
        var link = new ScriptedLink().Answers("TOS64-ANS/1 refused=unknown-verb seq=1");
        var transcript = Session("WOBBLE\n", link, out _);

        Assert.Equal(Cmd.UnknownVerbId, (ushort)link.Sent[0][14 + Cmd.VerbOffset + 1]);
        Assert.Contains("REFUSED: unknown-verb", transcript);
    }

    [Fact]
    public void ASharedStatusAndOutputLineKeepsTheTwoFieldsApart()
    {
        // `status=` and `out=` both run to the end of a line, so a parser that
        // handled them in the wrong order would fold one into the other. The
        // board never emits both, and this end is asserted not to confuse them
        // if a capture ever shows one.
        var answer = Cmd.Parse("TOS64-ANS/1 verb=STATUS seq=3 ok=1 status=TOS64-RESULT/1 ok=true");
        Assert.NotNull(answer);
        Assert.Equal("TOS64-RESULT/1 ok=true", answer!.Status);
        Assert.Null(answer.Output);

        var shell = Cmd.Parse("TOS64-ANS/1 verb=SHELL seq=3 ok=1 out=A:\\\\> more=7");
        Assert.NotNull(shell);
        Assert.Equal("A:\\>", shell!.Output);
        Assert.Equal(7u, shell.Withheld);
        Assert.Null(shell.Status);
    }
}
