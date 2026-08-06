namespace Netboot.Tests;

/// Reads `work/tools/netboot/server-address-cases.tsv`, the table both this
/// suite and `os/src/xtask`'s `board_run` tests assert against.
///
/// The repository root is found by walking up for `agent.md` rather than by a
/// relative path from the test binary, because the binary's location is a
/// property of the build configuration and a mirror test that silently stops
/// finding its table is worse than no mirror test at all — which is why a
/// missing file throws here rather than yielding an empty sequence.
internal static class SharedCases
{
    internal static IEnumerable<(string Value, string Verdict, string Why)> Load()
    {
        var path = Path.Combine(RepositoryRoot(), "work", "tools", "netboot",
            "server-address-cases.tsv");
        if (!File.Exists(path))
        {
            throw new FileNotFoundException($"the shared case table is missing: {path}");
        }

        foreach (var line in File.ReadAllLines(path).Skip(1))
        {
            if (line.Length == 0 || line[0] == '#') continue;
            var fields = line.Split('\t');
            if (fields.Length != 3)
            {
                throw new InvalidDataException($"not three fields: `{line}`");
            }
            yield return (fields[0], fields[1], fields[2]);
        }
    }

    private static string RepositoryRoot()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null)
        {
            if (File.Exists(Path.Combine(directory.FullName, "agent.md")))
            {
                return directory.FullName;
            }
            directory = directory.Parent;
        }
        throw new DirectoryNotFoundException(
            $"no agent.md above {AppContext.BaseDirectory}; cannot locate the repository root");
    }
}
