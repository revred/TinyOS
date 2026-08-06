using System.Net;

namespace Netboot.Tests;

/// `LE-97`: `tos64-netboot` started with `server address: 0.0.0.0` on
/// 2026-08-06 and would have handed the board `siaddr=0.0.0.0`, which fails to
/// fetch and looks exactly like a board fault. The only thing that caught it
/// was a human reading the line — and the tool prints `0.0.0.0` in the same
/// confident column it prints a real address.
///
/// The mechanism was `GuessLinkLocalAddress` taking the first `169.254.x.x` on
/// an interface that is UP. The bench Ethernet is `Disconnected` until the
/// board powers, because link needs a powered far end, so **on the very run
/// that needs the address the address cannot be discovered.**
///
/// It is worse once the link IS up: this bench host holds FOUR link-local
/// addresses (Ethernet, Bluetooth PAN, two virtual adapters), so first-one-wins
/// is a coin toss even in the working case and the wrong pick fails identically
/// with a plausible address in the log.
///
/// So the rule these tests hold is not "guess better". It is that **a server
/// which cannot name its own address refuses to serve**: every outcome that is
/// not a single unambiguous address carries no address at all, and nothing a
/// refusal produces can reach the wire.
public sealed class ServerAddressTests
{
    private static IPAddress Ip(string text) => IPAddress.Parse(text);

    /// An operator who names the address gets that address, whatever the NICs
    /// say. This is the escape hatch the 2026-08-06 session used to recover the
    /// boot (`--server 169.254.113.248`), so it must survive any later change
    /// to discovery.
    [Fact]
    public void An_explicit_address_is_used_verbatim_and_discovery_is_not_consulted()
    {
        var choice = ServerAddress.Choose(
            "169.254.113.248",
            [Ip("169.254.113.248"), Ip("169.254.8.8")]);

        Assert.Equal(ServerAddressOutcome.Named, choice.Outcome);
        Assert.Equal(Ip("169.254.113.248"), choice.Address);
    }

    /// **The last way `LE-97` can still happen.** Syntax was checked and
    /// existence was not: `--server=169.254.113.249`, one digit off the real
    /// one, passes four octets, digits, range and the not-`0.0.0.0` rule, goes
    /// into the DHCP `OFFER`, and the board fails to fetch **looking exactly
    /// like a board fault** — which is the whole of `LE-97`.
    ///
    /// It is a warning and not a refusal, deliberately. The NIC has no link
    /// until the board powers, so on the run that matters there is nothing to
    /// check against; a hard check would refuse the only working use case. So:
    /// **when there is evidence, use it; when there is none, stay quiet.**
    [Fact]
    public void A_named_address_that_is_not_on_this_host_is_flagged_but_still_served()
    {
        var choice = ServerAddress.Choose(
            "169.254.113.249",
            [Ip("169.254.113.248"), Ip("169.254.8.8")]);

        Assert.Equal(ServerAddressOutcome.NamedUnseen, choice.Outcome);
        Assert.True(choice.CanServe, "a warning must not become a refusal");
        Assert.Equal(Ip("169.254.113.249"), choice.Address);
        Assert.Contains("169.254.113.248", choice.Explain(), StringComparison.Ordinal);
    }

    /// The quiet case: nothing at all is enumerable, so there is no evidence to
    /// contradict the operator and nothing is said.
    ///
    /// **This branch is nearly unreachable on Windows and that is the point of
    /// the test below it.** `Loopback Pseudo-Interface 1` is always `Up` with
    /// `127.0.0.1`, so a check that stays quiet only when the host holds *no*
    /// address stays quiet never — it warns on every correct run instead.
    [Fact]
    public void A_named_address_is_not_flagged_when_there_is_nothing_to_check_it_against()
    {
        var choice = ServerAddress.Choose("169.254.113.249", []);

        Assert.Equal(ServerAddressOutcome.Named, choice.Outcome);
        Assert.Equal(Ip("169.254.113.249"), choice.Address);
    }

    /// **The cold `board-run`, which is the run this whole check exists for.**
    ///
    /// Observed on the bench 2026-08-06: a DISCONNECTED adapter is reported by
    /// `NetworkInformation` as `Down` and **still carries its APIPA address** —
    /// Bluetooth PAN held `169.254.92.35` while `Get-NetAdapter` said
    /// `Disconnected`. So the bench Ethernet's address is enumerable before the
    /// board powers, and the address a disconnected NIC still holds is exactly
    /// the "recorded bench address" this check needs: the OS already recorded
    /// it.
    ///
    /// Filtering the existence check to interfaces that are UP would have made
    /// the cold path warn on every correct run — the host would report only
    /// WiFi and loopback, neither of which is the bench address — which is the
    /// training effect that makes the one warning that matters unreadable.
    [Fact]
    public void The_bench_address_is_evidence_before_the_board_powers_even_though_its_link_is_down()
    {
        HostAddress[] cold =
        [
            new(Ip("127.0.0.1"), LinkIsUp: true),          // always up, always useless
            new(Ip("192.168.1.225"), LinkIsUp: true),      // WiFi
            new(Ip("169.254.113.248"), LinkIsUp: false),   // the bench NIC, board unpowered
        ];

        var right = ServerAddress.Choose(
            "169.254.113.248",
            ServerAddress.LinkLocalCandidates(cold),
            ServerAddress.KnownHostAddresses(cold));
        Assert.Equal(ServerAddressOutcome.Named, right.Outcome);

        var typo = ServerAddress.Choose(
            "169.254.113.249",
            ServerAddress.LinkLocalCandidates(cold),
            ServerAddress.KnownHostAddresses(cold));
        Assert.Equal(ServerAddressOutcome.NamedUnseen, typo.Outcome);
        Assert.True(typo.CanServe);
    }

    /// The two enumerations must not collapse into one. Discovery keeps the
    /// link-up requirement — an address on a dead adapter is not a link the
    /// board can be cabled to, and admitting them would turn a cold bench from
    /// `NoCandidate` into `Ambiguous` and a warm one from `Discovered` into
    /// `Ambiguous`. The existence check drops it, for the test above.
    [Fact]
    public void Discovery_requires_a_live_link_and_the_existence_check_does_not()
    {
        HostAddress[] cold =
        [
            new(Ip("127.0.0.1"), LinkIsUp: true),
            new(Ip("192.168.1.225"), LinkIsUp: true),
            new(Ip("169.254.113.248"), LinkIsUp: false),
            new(Ip("169.254.92.35"), LinkIsUp: false),
        ];

        Assert.Empty(ServerAddress.LinkLocalCandidates(cold));
        Assert.Contains(Ip("169.254.113.248"), ServerAddress.KnownHostAddresses(cold));
        Assert.Equal(
            ServerAddressOutcome.NoCandidate,
            ServerAddress.Choose(null, ServerAddress.LinkLocalCandidates(cold)).Outcome);
    }

    /// Loopback is never a "did you mean one of these" answer — it is in the
    /// refused set — so it is not evidence either. Left in, every warning would
    /// offer the operator the one address the tool would refuse.
    [Fact]
    public void Loopback_is_not_evidence_that_an_address_exists()
    {
        HostAddress[] only = [new(Ip("127.0.0.1"), LinkIsUp: true)];

        Assert.Empty(ServerAddress.KnownHostAddresses(only));
        Assert.Equal(
            ServerAddressOutcome.Named,
            ServerAddress.Choose("169.254.113.249", [], ServerAddress.KnownHostAddresses(only))
                .Outcome);
    }

    /// `Candidates` means one thing everywhere: the link-local addresses
    /// discovery considered. The warning's "did you mean one of these" list is
    /// a separate field, because a field with two meanings is read wrong by
    /// whichever caller was written second.
    [Fact]
    public void Candidates_is_always_the_link_local_set_and_never_the_host_set()
    {
        HostAddress[] host =
        [
            new(Ip("192.168.1.225"), LinkIsUp: true),
            new(Ip("169.254.7.7"), LinkIsUp: true),
        ];

        var choice = ServerAddress.Choose(
            "10.9.9.9",
            ServerAddress.LinkLocalCandidates(host),
            ServerAddress.KnownHostAddresses(host));

        Assert.Equal(ServerAddressOutcome.NamedUnseen, choice.Outcome);
        Assert.Equal([Ip("169.254.7.7")], choice.Candidates);
        Assert.Contains(Ip("192.168.1.225"), choice.HostAddresses);
        Assert.Contains("192.168.1.225", choice.Explain(), StringComparison.Ordinal);
    }

    /// A named address on a NIC that is up but not link-local is still an
    /// address this host holds. `Candidates` is the link-local filter's output,
    /// so the existence check gets its own enumeration — otherwise a bench on a
    /// real subnet is warned about every correct run.
    [Fact]
    public void A_named_routable_address_this_host_holds_is_not_flagged()
    {
        var choice = ServerAddress.Choose(
            "192.168.1.20",
            [Ip("169.254.7.7")],
            [Ip("192.168.1.20"), Ip("169.254.7.7")]);

        Assert.Equal(ServerAddressOutcome.Named, choice.Outcome);
    }

    /// The explicit and discovery paths used to disagree about what an address
    /// may be: discovery filtered hard to 169.254/16 while `--server` accepted
    /// anything four octets long that was not `0.0.0.0`.
    ///
    /// Routable unicast stays allowed — a bench on a real subnet is a real
    /// case, and nothing here knows the topology. But loopback, multicast and
    /// broadcast are the category the enum's own words already describe: *not
    /// somewhere a client can fetch from.* `127.0.0.1` would tell the board to
    /// fetch **from itself**.
    [Theory]
    [InlineData("127.0.0.1")]
    [InlineData("127.255.255.254")]
    [InlineData("224.0.0.1")]
    [InlineData("239.255.255.250")]
    [InlineData("240.0.0.1")]
    [InlineData("255.255.255.255")]
    [InlineData("0.1.2.3")]
    public void An_address_no_client_could_fetch_from_is_refused(string text)
    {
        var choice = ServerAddress.Choose(text, []);

        Assert.Equal(ServerAddressOutcome.Unusable, choice.Outcome);
        Assert.Null(choice.Address);
    }

    /// The exact value that nearly poisoned the boot, requested deliberately.
    ///
    /// `IPAddress.Any` is a bind wildcard and is not an address any client can
    /// route to, so it is refused on the way IN as well as on the way out —
    /// otherwise the fix would only cover the path that produced it once.
    [Fact]
    public void An_explicit_unspecified_address_is_refused_rather_than_honoured()
    {
        var choice = ServerAddress.Choose("0.0.0.0", [Ip("169.254.7.7")]);

        Assert.Equal(ServerAddressOutcome.Unusable, choice.Outcome);
        Assert.Null(choice.Address);
    }

    /// A typo used to reach the wire as an unhandled `FormatException` mid-way
    /// through argument parsing. Refusing it is the same class of fix as the
    /// rest of this file: the tool declines rather than continuing on a value
    /// it cannot justify.
    ///
    /// `169.254.113` is the interesting case and it is here because this test
    /// FOUND it: **`IPAddress.TryParse` accepts it** and returns 169.254.0.113.
    /// .NET still honours the historical shorthand forms, so a truncated
    /// address does not fail — it becomes a different, valid, wrong address,
    /// and the tool would then have printed that confidently and served
    /// nothing. Same shape as `LE-97` itself, one layer down.
    [Theory]
    [InlineData("169.254.113")]        // three octets: parses as 169.254.0.113
    [InlineData("169.254")]            // two: parses as 169.0.0.254
    [InlineData("not-an-address")]
    [InlineData("")]
    public void A_malformed_explicit_address_is_refused_rather_than_thrown(string text)
    {
        var choice = ServerAddress.Choose(text, [Ip("169.254.7.7")]);

        Assert.Equal(ServerAddressOutcome.Malformed, choice.Outcome);
        Assert.Null(choice.Address);
    }

    /// The other half of the shorthand check: a correct four-octet address must
    /// still be accepted, or the guard above would have made `--server`
    /// unusable and the refusal unescapable.
    [Fact]
    public void A_four_octet_address_still_parses()
    {
        var choice = ServerAddress.Choose("169.254.0.113", []);

        Assert.Equal(ServerAddressOutcome.Named, choice.Outcome);
        Assert.Equal(Ip("169.254.0.113"), choice.Address);
    }

    /// The 2026-08-06 case exactly: the board is unpowered, the bench NIC is
    /// `Disconnected`, discovery finds nothing. The old code answered
    /// `IPAddress.Any` and started anyway.
    [Fact]
    public void No_candidate_refuses_instead_of_falling_back_to_Any()
    {
        var choice = ServerAddress.Choose(null, []);

        Assert.Equal(ServerAddressOutcome.NoCandidate, choice.Outcome);
        Assert.Null(choice.Address);
    }

    /// Discovery is still allowed to work — a refusal that fires on a clean
    /// bench is a refusal nobody keeps. One unambiguous candidate is a decision,
    /// not a guess.
    [Fact]
    public void Exactly_one_link_local_candidate_is_accepted()
    {
        var choice = ServerAddress.Choose(null, [Ip("169.254.113.248")]);

        Assert.Equal(ServerAddressOutcome.Discovered, choice.Outcome);
        Assert.Equal(Ip("169.254.113.248"), choice.Address);
    }

    /// The failure nobody had noticed: four link-local addresses on this host
    /// and the old code took the first. A coin toss that prints a plausible
    /// address is worse than one that prints `0.0.0.0`, because nothing in the
    /// log looks wrong.
    [Fact]
    public void Several_link_local_candidates_are_ambiguous_and_all_of_them_are_reported()
    {
        var choice = ServerAddress.Choose(
            null,
            [Ip("169.254.113.248"), Ip("169.254.9.1"), Ip("169.254.22.7"), Ip("169.254.3.3")]);

        Assert.Equal(ServerAddressOutcome.Ambiguous, choice.Outcome);
        Assert.Null(choice.Address);
        Assert.Equal(4, choice.Candidates.Count);
    }

    /// A routable address is not the bench link. Filtering here rather than in
    /// the NIC walk means a later change to enumeration cannot smuggle one in.
    [Fact]
    public void Addresses_outside_169_254_are_not_candidates_at_all()
    {
        var choice = ServerAddress.Choose(
            null,
            [Ip("192.168.1.20"), Ip("10.0.0.5"), Ip("169.254.113.248")]);

        Assert.Equal(ServerAddressOutcome.Discovered, choice.Outcome);
        Assert.Equal(Ip("169.254.113.248"), choice.Address);
    }

    /// One address enumerated twice is one address. Otherwise a host that
    /// reports the same NIC through two providers would be refused for an
    /// ambiguity that does not exist.
    [Fact]
    public void The_same_address_twice_is_one_candidate_and_not_an_ambiguity()
    {
        var choice = ServerAddress.Choose(
            null,
            [Ip("169.254.113.248"), Ip("169.254.113.248")]);

        Assert.Equal(ServerAddressOutcome.Discovered, choice.Outcome);
        Assert.Equal(Ip("169.254.113.248"), choice.Address);
    }

    /// The invariant the whole file exists for, stated once over every outcome:
    /// **no refusal carries an address.** A future outcome added without an
    /// address rule fails here rather than on a bench at the cost of a power
    /// cycle.
    [Fact]
    public void Every_outcome_that_is_not_an_acceptance_carries_no_address()
    {
        ServerAddressChoice[] refusals =
        [
            ServerAddress.Choose("0.0.0.0", [Ip("169.254.7.7")]),
            ServerAddress.Choose("not-an-address", []),
            ServerAddress.Choose(null, []),
            ServerAddress.Choose(null, [Ip("169.254.7.7"), Ip("169.254.8.8")]),
            ServerAddress.Choose(null, [Ip("192.168.1.20")]),
        ];

        foreach (var refusal in refusals)
        {
            Assert.False(refusal.CanServe);
            Assert.Null(refusal.Address);
            Assert.NotEmpty(refusal.Explain());
        }
    }

    /// The one test that touches the real machine, because the walk and its two
    /// projections are a seam and `LE-66` says a declared-thin seam with no test
    /// is not thin, it is untested.
    ///
    /// It asserts only what is true of any host: loopback exists, it is never
    /// evidence, and discovery never offers something that is not link-local.
    /// Nothing here depends on this bench's addresses.
    [Fact]
    public void The_real_host_walk_feeds_both_projections_without_leaking_loopback()
    {
        var host = ServerAddress.EnumerateHost();

        Assert.NotEmpty(host);
        Assert.Contains(host, a => a.Address.Equals(IPAddress.Loopback));
        Assert.DoesNotContain(IPAddress.Loopback, ServerAddress.KnownHostAddresses(host));
        Assert.All(
            ServerAddress.LinkLocalCandidates(host),
            a => Assert.StartsWith("169.254.", a.ToString(), StringComparison.Ordinal));
    }

    /// `Explain`'s acceptance arms were unreachable and therefore untested:
    /// `Program` called it only under `if (!choice.CanServe)` and printed the
    /// raw outcome on the success path. Dead strings in a file whose whole
    /// subject is a tool saying what it chose.
    [Fact]
    public void An_acceptance_explains_itself_and_names_the_address()
    {
        foreach (var choice in new[]
                 {
                     ServerAddress.Choose("169.254.113.248", []),
                     ServerAddress.Choose(null, [Ip("169.254.113.248")]),
                     ServerAddress.Choose("169.254.113.248", [Ip("169.254.9.9")]),
                 })
        {
            Assert.True(choice.CanServe);
            Assert.Contains("169.254.113.248", choice.Explain(), StringComparison.Ordinal);
        }
    }

    /// The mirror `08C` argued for and did not build. `board_run::server_address`
    /// implements these same rules in Rust, in a separate program that nothing
    /// links to this one, and the Rust suite reads the same file. Duplication is
    /// the right call across this seam — but an unasserted duplicate is two
    /// implementations that agree today, which is what `TransferBeacon` and
    /// `TransferGuard` got a mirror test to avoid.
    [Fact]
    public void Every_shared_case_gets_the_verdict_the_mirror_file_states()
    {
        var checked_ = 0;
        foreach (var (value, verdict, why) in SharedCases.Load())
        {
            var choice = ServerAddress.Choose(value, []);
            var accepted = choice.CanServe;
            Assert.True(
                accepted == (verdict == "accept"),
                $"`{value}` should {verdict} ({why}) but Choose returned {choice.Outcome}");
            checked_++;
        }
        // Nothing was wrong vs nothing was looked at — the distinction the
        // bound-provenance checker keeps for the same reason.
        Assert.True(checked_ >= 20, $"only {checked_} shared cases were read");
    }

    /// A refusal has to say what to do next or it is just a stop. Every one of
    /// them names `--server`, which is the single action that resolves all of
    /// them.
    [Fact]
    public void Every_refusal_names_the_flag_that_resolves_it()
    {
        ServerAddressChoice[] refusals =
        [
            ServerAddress.Choose("0.0.0.0", []),
            ServerAddress.Choose("not-an-address", []),
            ServerAddress.Choose(null, []),
            ServerAddress.Choose(null, [Ip("169.254.7.7"), Ip("169.254.8.8")]),
        ];

        foreach (var refusal in refusals)
        {
            Assert.Contains("--server", refusal.Explain(), StringComparison.Ordinal);
        }
    }
}
