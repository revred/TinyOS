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

        Assert.Equal(Cmd.PayloadBytes, constants["COMMAND_PAYLOAD_BYTES"]);
        // The offsets the C# builder writes at, checked against the widths the
        // Rust side declares — an envelope that agreed on names and disagreed
        // on offsets would produce `wrong-magic` for every command sent.
        Assert.Equal(6, Cmd.MagicOffset);
        Assert.Equal(10, Cmd.VerbOffset);
        Assert.Equal(12, Cmd.SequenceOffset);
        Assert.Equal(16, Cmd.ArgumentOffset);
        Assert.Equal(Cmd.PayloadBytes - Cmd.ArgumentOffset, constants["ARGUMENT_BYTES"]);
    }
}
