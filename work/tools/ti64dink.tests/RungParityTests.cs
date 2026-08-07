// LE-80's real close: something fails when the kernel adds a rung and the
// host does not learn it.
//
// The 2026-08-05 fix made the C# rung table carry the (target, name, category,
// action) tuple and deleted the second hand-kept list — but its own row said
// what remained open: "nothing yet FAILS when the kernel adds a rung and the
// host does not learn it. The parity test guarding the hal-arm64/kernel pair
// has no counterpart across the Rust/C# boundary." This is that counterpart:
// it reads `kernel::spoor_stream` — the Rust source itself, the same authority
// the kernel compiles — and holds the ti64dink table against it in both
// directions. A rung added in Rust with no C# row fails here; a C# row with no
// Rust variant, or with a drifted (category, action) pair, fails here.
//
// A source-reading test is the row's own named close ("a generated table, or a
// test reading the Rust source, is the real close"). It is deliberately
// brittle against the file MOVING — a rename breaks it loudly, which is the
// acceptable direction; what it can never do is pass while the vocabularies
// disagree.

using System.Text.RegularExpressions;

public sealed partial class RungParityTests
{
    private const string RustSource = "os/src/kernel/src/spoor_stream.rs";

    private sealed record RustRung(int Id, string Name, string Category, string Action);

    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null)
        {
            if (File.Exists(Path.Combine(dir.FullName, "agent.md")))
            {
                return dir.FullName;
            }
            dir = dir.Parent!;
        }
        throw new InvalidOperationException("repository root (agent.md) not found above test bin");
    }

    [GeneratedRegex(@"^\s*(\w+) = (\d+),\s*$", RegexOptions.Multiline)]
    private static partial Regex VariantRegex();

    [GeneratedRegex(@"((?:Rung::\w+\s*(?:\|\s*)?)+)=>\s*\(Category::(\w+),\s*Action::(\w+)\)")]
    private static partial Regex ArmRegex();

    [GeneratedRegex(@"Rung::(\w+)")]
    private static partial Regex RungNameRegex();

    private static List<RustRung> ReadKernelVocabulary()
    {
        var source = File.ReadAllText(Path.Combine(RepoRoot(), RustSource));
        // Comment lines are prose, and prose mentions vocabulary names — the
        // exact trap ci_gates.rs records about display names. Strip them.
        var code = string.Join('\n',
            source.Split('\n').Where(line => !line.TrimStart().StartsWith("//")));

        var enumStart = code.IndexOf("pub enum Rung {", StringComparison.Ordinal);
        Assert.True(enumStart >= 0, $"`pub enum Rung` not found in {RustSource} — moved?");
        var enumBody = code[enumStart..code.IndexOf("\n}", enumStart, StringComparison.Ordinal)];
        var ids = VariantRegex().Matches(enumBody)
            .ToDictionary(m => m.Groups[1].Value, m => int.Parse(m.Groups[2].Value));
        Assert.NotEmpty(ids);

        var taxonomyStart = code.IndexOf("pub const fn taxonomy", StringComparison.Ordinal);
        Assert.True(taxonomyStart >= 0, $"`fn taxonomy` not found in {RustSource} — moved?");
        var taxonomyBody =
            code[taxonomyStart..code.IndexOf("\n    }", taxonomyStart, StringComparison.Ordinal)];

        var rungs = new List<RustRung>();
        foreach (Match arm in ArmRegex().Matches(taxonomyBody))
        {
            foreach (Match name in RungNameRegex().Matches(arm.Groups[1].Value))
            {
                var variant = name.Groups[1].Value;
                Assert.True(ids.ContainsKey(variant), $"taxonomy names unknown variant {variant}");
                rungs.Add(new RustRung(
                    ids[variant], variant, arm.Groups[2].Value, arm.Groups[3].Value));
            }
        }
        // Every enum variant must appear in taxonomy — Rust's own match
        // exhaustiveness guarantees it, so a shortfall here is a parse bug in
        // THIS test, and it must fail rather than quietly compare less.
        Assert.Equal(ids.Count, rungs.Count);
        return rungs;
    }

    [Fact]
    public void every_kernel_rung_has_an_identical_ti64dink_row()
    {
        foreach (var rust in ReadKernelVocabulary())
        {
            var row = Program.Rungs.SingleOrDefault(r => r.Target == rust.Id);
            Assert.True(row != default,
                $"kernel rung {rust.Name}={rust.Id} has no ti64dink row — the decoder would " +
                $"print target={rust.Id} and `--until rung={rust.Name}` would watch a live " +
                "stream and exit 1 as a timeout, which is LE-80 verbatim");
            Assert.Equal(rust.Name, row.Name);
            Assert.Equal(rust.Category, row.Category);
            Assert.Equal(rust.Action, row.Action);
        }
    }

    [Fact]
    public void every_ti64dink_row_is_a_kernel_rung()
    {
        var kernel = ReadKernelVocabulary();
        foreach (var row in Program.Rungs)
        {
            Assert.True(kernel.Any(rust => rust.Id == row.Target && rust.Name == row.Name),
                $"ti64dink row {row.Name}={row.Target} does not exist in the kernel — the " +
                "watch would accept a condition the board can never emit");
        }
    }

    [Fact]
    public void no_rung_has_id_zero()
    {
        // `target` is 0 on paths that carry no rung; a rung with id 0 would
        // make every such record nameable, which is the masquerade the
        // (category, action) guard exists to prevent.
        Assert.DoesNotContain(Program.Rungs, row => row.Target == 0);
        Assert.DoesNotContain(ReadKernelVocabulary(), rust => rust.Id == 0);
    }
}
