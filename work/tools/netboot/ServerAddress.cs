using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;

/// What `tos64-netboot` decided about its own address, and why (`LE-97`).
///
/// `Named`, `NamedUnseen` and `Discovered` are the acceptances. Everything else
/// is a refusal and carries no address, which is the whole point: the original
/// code answered `IPAddress.Any` for every refusal case and started anyway.
internal enum ServerAddressOutcome
{
    /// The operator named it with `--server`.
    Named,

    /// The operator named it, **and this host does not appear to hold it**.
    /// An acceptance, not a refusal — see [`ServerAddress.Choose`] for why the
    /// one run that matters has no evidence to check against.
    NamedUnseen,

    /// Exactly one link-local address existed and it was taken.
    Discovered,

    /// `--server` was given but is not an IPv4 address.
    Malformed,

    /// `--server` parsed but cannot be served FROM: the wildcard `0.0.0.0`,
    /// `0/8`, loopback, multicast, reserved, or the limited broadcast.
    /// Routable unicast is not in this set — a bench may be on a real subnet.
    Unusable,

    /// No link-local address exists on any interface that is up. On this bench
    /// that is the NORMAL state before the board powers, because link needs a
    /// powered far end.
    NoCandidate,

    /// More than one link-local address exists and nothing here can tell which
    /// one the board is cabled to.
    Ambiguous,
}

/// One IPv4 address this host holds, and whether its interface has a live link.
///
/// The pair is the seam: the NIC walk produces it, and every decision that
/// depends on link state is a pure function of a list of these. Windows keeps a
/// disconnected adapter's APIPA address, so `LinkIsUp: false` with a real
/// address is the normal cold-bench state, not a contradiction.
internal readonly record struct HostAddress(IPAddress Address, bool LinkIsUp);

/// One resolution attempt: the verdict, the address if there is one, the
/// link-local candidates discovery considered, the host addresses the existence
/// check was made against, and the specific reason when the outcome alone does
/// not carry it.
///
/// `Candidates` and `HostAddresses` are separate fields on purpose. `Candidates`
/// means exactly one thing everywhere — what discovery could have chosen — while
/// the warning needs "did you mean one of these", which includes routable
/// addresses discovery would never pick. One field carrying both meanings is
/// read wrong by whichever caller was written second.
internal readonly record struct ServerAddressChoice(
    ServerAddressOutcome Outcome,
    IPAddress? Address,
    IReadOnlyList<IPAddress> Candidates,
    IReadOnlyList<IPAddress> HostAddresses,
    string Reason = "")
{
    /// Whether this choice can be served from at all. The only property a
    /// caller should branch on.
    internal bool CanServe => Address is not null;

    /// Whether the operator should be told something even though the run
    /// proceeds. Separate from [`CanServe`] so a warning can never quietly
    /// become a refusal, nor a refusal a warning.
    internal bool IsWarning => Outcome == ServerAddressOutcome.NamedUnseen;

    /// The verdict in the operator's terms, including what to do about it.
    ///
    /// Every refusal names `--server`, because that is the one action that
    /// resolves all of them — a stop with no next step gets worked around
    /// rather than fixed. The acceptances explain themselves too: `Program`
    /// prints this on every path, so none of these arms is dead.
    internal string Explain() => Outcome switch
    {
        ServerAddressOutcome.Named =>
            $"server address named on the command line: {Address}",
        ServerAddressOutcome.Discovered =>
            $"server address discovered, one candidate only: {Address}",
        ServerAddressOutcome.NamedUnseen =>
            $"{Address} was named, but this host does not appear to hold it. Addresses found: "
            + string.Join(", ", HostAddresses)
            + ". This is NOT the board-is-not-powered-yet case — a disconnected adapter keeps "
            + "its address and is counted above — so check the digits: an address one digit off "
            + "passes every syntax rule, enters the DHCP OFFER, and the board then fails to "
            + "fetch looking exactly like a board fault (LE-97).",
        ServerAddressOutcome.Malformed =>
            $"{Reason} Pass --server <ip> with the bench interface's own address.",
        ServerAddressOutcome.Unusable =>
            $"{Reason} Pass --server <ip> with the bench interface's own address.",
        ServerAddressOutcome.NoCandidate =>
            "no 169.254.x.x address on any interface that is up. This is the EXPECTED state "
            + "before the board powers — link needs a powered far end — so the address cannot "
            + "be discovered on the run that needs it. Pass --server <ip> with the bench "
            + "interface's own address.",
        ServerAddressOutcome.Ambiguous =>
            $"{Candidates.Count} link-local addresses exist and nothing here can tell which one "
            + "the board is cabled to; Bluetooth PAN and virtual adapters carry them too. "
            + "Pass --server <ip> to name the bench interface.",
        _ => "unrecognised outcome",
    };
}

/// Decides `tos64-netboot`'s own address, or refuses to (`LE-97`).
///
/// Pure and separated from the NIC walk so the decision is testable without a
/// bench, a board, or a power cycle — the seam `LE-66` asks for.
///
/// # The two paths agree about what an address may be
///
/// Discovery filters candidates to 169.254/16; the explicit path accepts
/// routable unicast as well, because a bench on a real subnet is a real case
/// and nothing here knows the topology — guessing it is `LE-97`'s mistake. What
/// **neither** path accepts is an address no client could fetch FROM: the
/// wildcard, `0/8`, loopback, multicast, reserved and the limited broadcast.
/// `127.0.0.1` would tell the board to fetch from itself.
///
/// # Existence is checked when it can be, and only then
///
/// Syntax is not enough: `--server=169.254.113.249`, one digit off the real
/// address, passes four octets, digits, range and every category rule, goes
/// into the DHCP `OFFER`, and the board fails to fetch **looking exactly like a
/// board fault** — which is all of `LE-97`.
///
/// It cannot be a refusal. `LE-97`'s own ordering trap says the NIC has no link
/// until the board powers, so on the run that matters there is nothing to check
/// against and a hard check would refuse the only working use case. So the rule
/// is: **when there is evidence, use it; when there is none, stay quiet.**
///
/// **What that does NOT cover, stated rather than implied:** on a cold
/// `xtask board-run` the server starts before power moves, so the link is down,
/// nothing is enumerable, and this check is silent — exactly the unattended run
/// where a mistyped address costs a mains cycle. It fires on the interactive
/// path (the 2026-08-06 recovery shape: restart the server with the board
/// already up) and on any `board-run` that follows another, since every run
/// leaves the board ON. The typo-on-a-cold-automated-run case remains open, and
/// closing it needs evidence that does not exist at that moment — a recorded
/// bench address, or a first run that learns one.
internal static class ServerAddress
{
    internal static ServerAddressChoice Choose(
        string? explicitAddress,
        IReadOnlyList<IPAddress> candidates)
        => Choose(explicitAddress, candidates, candidates);

    /// `hostAddresses` is what [`KnownHostAddresses`] returned — every address
    /// this host holds that a client could conceivably fetch from, link up or
    /// not. `candidates` is what [`LinkLocalCandidates`] returned. They are
    /// separate arguments because they answer different questions and one of
    /// them requires a live link.
    internal static ServerAddressChoice Choose(
        string? explicitAddress,
        IReadOnlyList<IPAddress> candidates,
        IReadOnlyList<IPAddress> hostAddresses)
    {
        var linkLocal = candidates.Where(IsBenchLinkLocal).Distinct().ToArray();
        var known = hostAddresses.Where(CouldBeAServer).Distinct().ToArray();

        if (explicitAddress is not null)
        {
            var (parsed, refusal, reason) = ParseExplicit(explicitAddress);
            if (parsed is null)
            {
                return new ServerAddressChoice(refusal, null, linkLocal, known, reason);
            }

            // Quiet only when there is genuinely nothing to check against. That
            // branch is nearly unreachable on Windows — loopback is always up —
            // which is exactly why loopback is excluded from `known` and why
            // `known` does not require a live link: otherwise "quiet when there
            // is no evidence" degrades into "warn on every correct cold run".
            var outcome = known.Length > 0 && !known.Contains(parsed)
                ? ServerAddressOutcome.NamedUnseen
                : ServerAddressOutcome.Named;
            return new ServerAddressChoice(outcome, parsed, linkLocal, known);
        }

        return linkLocal.Length switch
        {
            0 => new ServerAddressChoice(ServerAddressOutcome.NoCandidate, null, linkLocal, known),
            1 => new ServerAddressChoice(
                ServerAddressOutcome.Discovered, linkLocal[0], linkLocal, known),
            _ => new ServerAddressChoice(ServerAddressOutcome.Ambiguous, null, linkLocal, known),
        };
    }

    /// What discovery may choose from: link-local, **and the link must be up**.
    ///
    /// The link-up requirement stays here and only here. An address on a dead
    /// adapter is not a link the board can be cabled to, and admitting them
    /// would turn a cold bench from `NoCandidate` into `Ambiguous` and a warm
    /// one from `Discovered` into `Ambiguous` — a refusal in both cases, but
    /// the wrong one, naming addresses that cannot carry a boot.
    internal static IReadOnlyList<IPAddress> LinkLocalCandidates(
        IReadOnlyList<HostAddress> addresses)
        => addresses.Where(a => a.LinkIsUp && IsBenchLinkLocal(a.Address))
            .Select(a => a.Address)
            .Distinct()
            .ToArray();

    /// What the existence check counts as evidence: every address this host
    /// holds that could be a server, **whether or not its link is up**.
    ///
    /// Observed on the bench 2026-08-06: Windows keeps a disconnected adapter's
    /// APIPA address and `NetworkInformation` reports it, so the bench NIC's
    /// address is enumerable while the board is unpowered. That is what makes
    /// the check work on a cold `board-run` instead of crying wolf on it — and
    /// it is the "recorded bench address" the check needed, already recorded by
    /// the operating system.
    ///
    /// Loopback is excluded: it is in the refused set, so offering it as a "did
    /// you mean" is offering the one address the tool would reject — and on
    /// Windows it is always present, which would make the quiet branch dead.
    internal static IReadOnlyList<IPAddress> KnownHostAddresses(
        IReadOnlyList<HostAddress> addresses)
        => addresses.Where(a => CouldBeAServer(a.Address))
            .Select(a => a.Address)
            .Distinct()
            .ToArray();

    /// Whether an address is in the set this tool would ever serve from — the
    /// complement of `ParseExplicit`'s `Unusable` categories.
    private static bool CouldBeAServer(IPAddress address)
    {
        if (address.AddressFamily != AddressFamily.InterNetwork) return false;
        var b = address.GetAddressBytes();
        return b[0] is not (0 or 127) && b[0] < 224;
    }

    /// The rules an explicit `--server` value must satisfy. Mirrored by
    /// `board_run::server_address` in Rust; both sides assert the shared table
    /// in `server-address-cases.tsv`, because two separate programs that agree
    /// today are not the same thing as two that are held to agree.
    private static (IPAddress? Address, ServerAddressOutcome Outcome, string Reason) ParseExplicit(
        string text)
    {
        var octets = text.Split('.');
        if (octets.Length != 4)
        {
            return (null, ServerAddressOutcome.Malformed,
                "an IPv4 address is four dotted octets, and a shortened one PARSES as a "
                + "different valid address rather than failing.");
        }

        var value = new byte[4];
        for (var i = 0; i < 4; i++)
        {
            var octet = octets[i];
            if (octet.Length == 0 || !octet.All(char.IsAsciiDigit))
            {
                return (null, ServerAddressOutcome.Malformed,
                    "every octet must be plain decimal digits.");
            }
            // A leading zero is an octal ambiguity, and .NET's own parser
            // rejects it — so it is refused here rather than left to differ
            // from the Rust side, which validates before any power moves.
            if (octet.Length > 1 && octet[0] == '0')
            {
                return (null, ServerAddressOutcome.Malformed,
                    "a leading zero in an octet is an octal ambiguity.");
            }
            if (!byte.TryParse(octet, out value[i]))
            {
                return (null, ServerAddressOutcome.Malformed, "every octet must be 0-255.");
            }
        }

        var unusable = value[0] switch
        {
            _ when value is [0, 0, 0, 0] =>
                "0.0.0.0 is a bind wildcard, not an address a client can fetch from; a board "
                + "handed siaddr=0.0.0.0 fails to fetch and looks like a board fault (LE-97).",
            0 => "0.0.0.0/8 is \"this network\" and is not a source any client can reach.",
            127 => "127.0.0.0/8 is loopback — the board would be told to fetch from itself.",
            >= 224 and <= 239 => "224.0.0.0/4 is multicast, and a server address is one host.",
            >= 240 =>
                "240.0.0.0/4 is reserved and 255.255.255.255 is the limited broadcast; neither "
                + "is a host a client can fetch from.",
            _ => null,
        };
        return unusable is null
            ? (new IPAddress(value), ServerAddressOutcome.Named, "")
            : (null, ServerAddressOutcome.Unusable, unusable);
    }

    /// Link-local (169.254/16) IPv4 only, because that is what a point-to-point
    /// cable with no DHCP server produces on both ends and it is what the Pi is
    /// already using.
    private static bool IsBenchLinkLocal(IPAddress address)
    {
        if (address.AddressFamily != AddressFamily.InterNetwork) return false;
        var b = address.GetAddressBytes();
        return b[0] == 169 && b[1] == 254;
    }

    /// The NIC walk, and the only impure thing in this file.
    ///
    /// It records link state rather than filtering on it, because the two
    /// consumers disagree about whether a dead link disqualifies an address and
    /// **that disagreement is the fix**: discovery needs a live link, the
    /// existence check must not, and a walk that filtered would have forced
    /// them to share one answer.
    ///
    /// Every address is returned, from every interface — the original code
    /// returned the first link-local one and this host has four (Ethernet,
    /// Bluetooth PAN, two virtual adapters), so first-one-wins was a coin toss
    /// that printed a plausible address.
    internal static IReadOnlyList<HostAddress> EnumerateHost()
    {
        var found = new List<HostAddress>();
        foreach (var nic in NetworkInterface.GetAllNetworkInterfaces())
        {
            var up = nic.OperationalStatus == OperationalStatus.Up;
            foreach (var a in nic.GetIPProperties().UnicastAddresses)
            {
                if (a.Address.AddressFamily == AddressFamily.InterNetwork)
                {
                    found.Add(new HostAddress(a.Address, up));
                }
            }
        }
        return found;
    }
}
