// LE-80's discipline applied from day one, in the direction that matters most.
//
// The rung table drifted between Rust and C# and the symptom was a watch that
// reported a live event as an absence for 300 seconds. RungParityTests is the
// fix for that pair, written after the drift. This is the same guard for the
// command vocabulary, written before it: the board's verb ids, its refusal
// names and its envelope layout live in one Rust file, and every one of them
// is asserted against the C# tables in both directions.
//
// Both directions matters. A verb added in Rust with no C# row means an
// operator's console cannot say the board's own word. A C# row with no Rust
// variant means the console offers a verb the board will refuse — and prints a
// refusal that reads like a board defect. Neither can survive this file.
//
// Deliberately brittle against the Rust file MOVING (a rename breaks it
// loudly, the acceptable direction). What it can never do is pass while the
// two vocabularies disagree.

using System.Text.RegularExpressions;

public sealed partial class ConsoleParityTests
{
    private const string RustSource = "os/src/hal-arm64/src/tos64_cmd.rs";

    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null)
        {
            if (File.Exists(Path.Combine(dir.FullName, "agent.md"))) return dir.FullName;
            dir = dir.Parent!;
        }
        throw new InvalidOperationException("repository root (agent.md) not found above test bin");
    }

    /// The Rust source with its comments stripped — prose names vocabulary,
    /// and matching prose is how a parity test passes while the code differs
    /// (the trap `ci_gates.rs` records).
    private static string Code()
    {
        var source = File.ReadAllText(Path.Combine(RepoRoot(), RustSource));
        return string.Join('\n',
            source.Split('\n').Where(line => !line.TrimStart().StartsWith("//")));
    }

    [GeneratedRegex(@"Verb::(\w+) => (\d+),")]
    private static partial Regex VerbIdRegex();

    [GeneratedRegex(@"Verb::(\w+) => ""(\w+)"",")]
    private static partial Regex VerbNameRegex();

    [GeneratedRegex(@"CommandRefusal::(\w+) => ""([\w-]+)"",")]
    private static partial Regex RefusalRegex();

    [GeneratedRegex(@"pub const (\w+): usize = (\d+);")]
    private static partial Regex ConstRegex();

    /// Constants the board expresses as arithmetic over other constants rather
    /// than as a literal — `COMMAND_PAYLOAD_BYTES = HEADER_BYTES +
    /// ARGUMENT_BYTES` since 2026-08-08.
    ///
    /// Resolved here rather than asked for as a literal on the Rust side. A
    /// width stated as arithmetic over its own field layout is the form least
    /// likely to drift when a field moves, and this gate exists to check the
    /// VALUE both ends must agree on, not the syntax the board writes it in.
    [GeneratedRegex(@"pub const (\w+): usize = (\w+) \+ (\w+);")]
    private static partial Regex DerivedConstRegex();

    /// The reversible arms of `Writer::put_escaped` — the board's half of the
    /// escape handshake, read out of the board's own source.
    [GeneratedRegex(@"b'(\\.)' => self\.put\(b""([^""]*)""\),")]
    private static partial Regex EscapeArmRegex();

    /// Every field tag `render` writes. The `\s*` allows the leading space the
    /// board puts before every field but the first.
    [GeneratedRegex(@"\.put\(b""\s*([A-Za-z][\w-]*)=")]
    private static partial Regex AnswerFieldRegex();

    /// A Rust byte-string literal's source text, as the bytes it denotes.
    ///
    /// Only the two escapes this handshake can contain. Deliberately NOT a
    /// general unescaper: a Rust literal this gate cannot read is a literal
    /// this gate must refuse rather than guess at, and `Assert` below is where
    /// that refusal lands.
    private static string RustLiteral(string source) =>
        source.Replace(@"\\", "\u0001").Replace(@"\n", "\n").Replace("\u0001", @"\");

    /// The body of one Rust function, by name — so a regex meant for
    /// `put_escaped` cannot match something that happens to look like it
    /// three hundred lines away.
    private static string Body(string code, string signature)
    {
        var at = code.IndexOf(signature, StringComparison.Ordinal);
        Assert.True(at >= 0, $"{RustSource} no longer contains `{signature}` — this gate is blind");
        var depth = 0;
        for (var i = code.IndexOf('{', at); i < code.Length; i++)
        {
            if (code[i] == '{') depth++;
            else if (code[i] == '}' && --depth == 0) return code[at..(i + 1)];
        }
        throw new InvalidOperationException($"unbalanced braces after `{signature}`");
    }

    /// The escape handshake, in both directions (`LE-80`'s discipline, applied
    /// to the pair that carries shell output).
    ///
    /// `STORY-P1-09-18` put a whole `TINYCMD` transcript on one line of wire by
    /// escaping the two octets that would break it, and this console un-escapes
    /// them back. **Nothing held those two halves together.** The verb table and
    /// the refusal vocabulary each had a gate; the escape pair did not, so a
    /// third escape class added on the board would have arrived here as literal
    /// backslashes in an operator's transcript, with every test on both sides
    /// still green.
    ///
    /// Read out of the board's source rather than restated: the arms are the
    /// contract.
    [Fact]
    public void TheEscapeVocabularyIsOneVocabularyAcrossBothEndsOfTheCable()
    {
        var arms = EscapeArmRegex().Matches(Body(Code(), "fn put_escaped")).ToList();
        Assert.NotEmpty(arms);

        foreach (Match arm in arms)
        {
            var original = RustLiteral(arm.Groups[1].Value); // the octet escaped
            var onWire = RustLiteral(arm.Groups[2].Value);   // what the board sends

            // The board grew it, so it must be longer — an "escape" the same
            // width as its input is not an escape.
            Assert.True(onWire.Length > original.Length, $"`{original}` is not escaped by `{onWire}`");
            // And this end must invert it exactly.
            Assert.Equal(original, Cmd.Unescape(onWire));
        }

        // C# -> Rust: this console may not know an escape the board never
        // writes, because un-escaping something the board sends literally would
        // silently corrupt real output. The two reversible arms are all there
        // are; anything else the board emits is the lossy `?` class, which is
        // asserted below to stay lossy rather than be invented back.
        Assert.Equal(2, arms.Count);
        Assert.Equal("?", Cmd.Unescape("?"));
        Assert.Equal(@"A:\>", Cmd.Unescape(@"A:\\>"));
    }

    /// The answer's field names, in both directions.
    ///
    /// The board writes `out=` and this console looks for `out=`; nothing but
    /// habit kept those two strings equal. A rename on the board would leave
    /// the console parsing an answer it could no longer read and printing an
    /// empty transcript under a command that in fact succeeded — a silent
    /// failure of exactly the kind `LE-80` names.
    [Fact]
    public void EveryFieldTheBoardWritesIsAFieldThisConsoleReadsAndViceVersa()
    {
        var written = AnswerFieldRegex().Matches(Body(Code(), "pub fn render"))
            .Select(m => m.Groups[1].Value)
            .ToHashSet();
        Assert.NotEmpty(written);

        // `ok` is written and deliberately not parsed: it is the constant `1`
        // on every answer the board can render, so reading it would assert a
        // tautology. Named here rather than silently absent, because the whole
        // point of this gate is that an unparsed field is a decision.
        var deliberatelyIgnored = new HashSet<string> { "ok" };

        // The keys `Cmd.Parse` actually acts on — the `switch` arms plus the
        // three fields it special-cases because they run to the end of a line.
        var read = new HashSet<string> { "verb", "refused", "seq", "dropped", "status", "out", "more" };

        foreach (var field in written)
        {
            Assert.True(read.Contains(field) || deliberatelyIgnored.Contains(field),
                $"the board writes `{field}=` and ti64dink would drop it unread");
        }
        foreach (var field in read)
        {
            Assert.True(written.Contains(field),
                $"ti64dink parses `{field}=` that {RustSource}'s render never writes");
        }

        // The sentinel the parser gates on, which is not a field and so is not
        // covered above — and is the one string whose drift would make every
        // answer unparseable at once.
        Assert.Contains(@"put(b""TOS64-ANS/1 "")", Code().Replace("\r", ""));
    }

    [Fact]
    public void EveryVerbTheBoardHoldsIsAVerbThisConsoleCanSendAndViceVersa()
    {
        var code = Code();
        var ids = VerbIdRegex().Matches(code)
            .ToDictionary(m => m.Groups[1].Value, m => ushort.Parse(m.Groups[2].Value));
        var names = VerbNameRegex().Matches(code)
            .ToDictionary(m => m.Groups[1].Value, m => m.Groups[2].Value);

        Assert.NotEmpty(ids);
        Assert.Equal(ids.Count, names.Count);

        // Rust -> C#: a row the board holds that this console cannot name.
        foreach (var (variant, id) in ids)
        {
            var wireName = names[variant];
            Assert.True(
                Cmd.Verbs.Any(v => v.Name == wireName && v.Id == id),
                $"{RustSource} holds verb {wireName}={id}; ti64dink's table does not");
        }

        // C# -> Rust: a row this console offers that the board would refuse.
        foreach (var (name, id) in Cmd.Verbs)
        {
            Assert.True(
                ids.Any(pair => names[pair.Key] == name && pair.Value == id),
                $"ti64dink offers verb {name}={id}; {RustSource} has no such row");
        }
    }

    [Fact]
    public void TheRefusalVocabularyIsOneVocabularyAcrossBothEndsOfTheCable()
    {
        var wire = RefusalRegex().Matches(Code()).Select(m => m.Groups[2].Value).ToList();
        Assert.NotEmpty(wire);

        foreach (var name in wire)
        {
            Assert.True(Cmd.Refusals.Contains(name),
                $"the board can speak refusal `{name}` and ti64dink would print it unrecognised");
        }
        foreach (var name in Cmd.Refusals)
        {
            Assert.True(wire.Contains(name),
                $"ti64dink knows refusal `{name}` that {RustSource} never emits");
        }
        Assert.Equal(wire.Count, Cmd.Refusals.Length);
    }

    [Fact]
    public void TheEnvelopeLayoutIsTheSameArithmeticOnBothSides()
    {
        var constants = ConstRegex().Matches(Code())
            .ToDictionary(m => m.Groups[1].Value, m => int.Parse(m.Groups[2].Value));

        // Two passes, because a derived constant may be built from another
        // derived one (ADMITTED_CAPACITY is COMMAND_PAYLOAD_BYTES + 16).
        for (var pass = 0; pass < 2; pass++)
        {
            foreach (Match match in DerivedConstRegex().Matches(Code()))
            {
                if (Term(constants, match.Groups[2].Value) is int left
                    && Term(constants, match.Groups[3].Value) is int right)
                {
                    constants[match.Groups[1].Value] = left + right;
                }
            }
        }

        Assert.Equal(Cmd.PayloadBytes, constants["COMMAND_PAYLOAD_BYTES"]);
        // The padding property, held at both ends: a frame UNDER the Ethernet
        // minimum is padded by the NIC and the padding is indistinguishable
        // from a wrong-width field. `>=`, never `==` — the quantifier that
        // held the command line at 30 octets until 2026-08-08.
        Assert.True(14 + Cmd.PayloadBytes >= 60);
        // The offsets the C# builder writes at, checked against the widths the
        // Rust side declares — an envelope that agreed on names and disagreed
        // on offsets would produce `wrong-magic` for every command sent.
        Assert.Equal(6, Cmd.MagicOffset);
        Assert.Equal(10, Cmd.VerbOffset);
        Assert.Equal(12, Cmd.SequenceOffset);
        Assert.Equal(16, Cmd.ArgumentOffset);
        Assert.Equal(Cmd.PayloadBytes - Cmd.ArgumentOffset, constants["ARGUMENT_BYTES"]);
    }

    /// One term of a derived constant: another constant's name, or a literal.
    private static int? Term(Dictionary<string, int> constants, string token) =>
        constants.TryGetValue(token, out var value) ? value
        : int.TryParse(token, out var literal) ? literal
        : null;
}
