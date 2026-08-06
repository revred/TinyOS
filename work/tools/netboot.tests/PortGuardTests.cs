using System.Net;
using System.Net.Sockets;

namespace Netboot.Tests;

/// LE-87: two tos64-netboot instances bound UDP 69 at once and the stale one
/// won every TFTP request, so the board booted an image nobody meant to serve
/// while the new server logged a clean DHCP exchange and looked healthy.
///
/// The mechanism was SO_REUSEADDR. These tests hold BOTH halves of it: the
/// platform semantic that made a silent second bind possible, and the parser
/// that names who is holding the port -- because `netstat` showed two PIDs on
/// :69 while `Get-NetUDPEndpoint` reported one, which is why the first look did
/// not find it.
public sealed class PortGuardTests
{
    /// The defect itself, as a characterisation test of the platform.
    ///
    /// This is the behaviour tos64-netboot HAD, and it must be recorded rather
    /// than remembered: on Windows, SO_REUSEADDR lets a second process bind a
    /// UDP port that is already held, the bind SUCCEEDS SILENTLY, and the stack
    /// then delivers each datagram to exactly one of the two sockets. There is
    /// no error anywhere for an operator to see.
    [Fact]
    public void ReuseAddress_lets_a_second_bind_of_a_held_port_succeed_silently()
    {
        using var first = Reusable();
        var port = Bind(first);

        using var second = Reusable();
        var boom = Record.Exception(() => second.Bind(new IPEndPoint(IPAddress.Loopback, port)));

        Assert.Null(boom);
    }

    /// The fix. Without SO_REUSEADDR the second bind fails, and it fails with
    /// the one error code worth branching on.
    [Fact]
    public void Exclusive_bind_of_a_held_port_fails_with_AddressAlreadyInUse()
    {
        using var first = Reusable();
        var port = Bind(first);

        var boom = Assert.Throws<SocketException>(() => PortGuard.BindExclusive(IPAddress.Loopback, port));

        Assert.Equal(SocketError.AddressAlreadyInUse, boom.SocketErrorCode);
    }

    /// And it must not refuse a port that is merely unusual. A guard that
    /// cannot bind a free port is a guard nobody will keep.
    [Fact]
    public void Exclusive_bind_of_a_free_port_succeeds()
    {
        using var socket = PortGuard.BindExclusive(IPAddress.Loopback, 0);
        Assert.True(((IPEndPoint)socket.LocalEndPoint!).Port > 0);
    }

    /// The observation that diagnosed LE-87: TWO pids on one port. A parser
    /// that returns "the" holder would have reported the collision as a single
    /// ordinary listener -- which is what the tool that only checked
    /// `Get-NetUDPEndpoint` effectively did.
    [Fact]
    public void Two_holders_of_one_port_are_both_reported()
    {
        var pids = PortGuard.ParseUdpHolders(NetstatSample, 69);
        Assert.Equal(new[] { 12044, 23988 }, pids.Order().ToArray());
    }

    /// Ports are matched whole. `:6900` and `:690` are not `:69`, and a
    /// substring match would have named an innocent process on the one run
    /// where being wrong costs a power cycle.
    [Theory]
    [InlineData(690)]
    [InlineData(6900)]
    public void A_port_that_merely_starts_with_the_digits_is_a_different_port(int port)
    {
        var pids = PortGuard.ParseUdpHolders(NetstatSample, port);
        Assert.Single(pids);
        Assert.DoesNotContain(23988, pids);
    }

    /// TCP :69 is not UDP :69. The tool binds UDP; a TCP listener on the same
    /// number holds nothing it cares about.
    [Fact]
    public void Tcp_rows_are_not_udp_holders()
    {
        Assert.DoesNotContain(7777, PortGuard.ParseUdpHolders(NetstatSample, 69));
    }

    /// IPv6 rows spell the address `[::]`, so the port sits after a colon that
    /// is inside the brackets as well as the one that separates it.
    [Fact]
    public void Ipv6_rows_are_parsed_and_deduplicated_by_pid()
    {
        Assert.Equal(new[] { 4242 }, PortGuard.ParseUdpHolders(NetstatSample, 67).ToArray());
    }

    [Fact]
    public void An_unheld_port_has_no_holders()
    {
        Assert.Empty(PortGuard.ParseUdpHolders(NetstatSample, 6969));
    }

    /// Garbage in the middle of the output must not take the guard down: this
    /// runs at startup, and a parser that throws turns "refuse to start" into
    /// "crash at start" for a reason unrelated to the port.
    [Fact]
    public void Malformed_rows_are_skipped_rather_than_thrown_on()
    {
        const string junk = "  UDP    \n  UDP  0.0.0.0:69  *:*  not-a-pid\n  UDP    0.0.0.0:69   *:*   88\n";
        Assert.Equal(new[] { 88 }, PortGuard.ParseUdpHolders(junk, 69).ToArray());
    }

    private static Socket Reusable()
    {
        var s = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
        s.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.ReuseAddress, true);
        return s;
    }

    private static int Bind(Socket s)
    {
        s.Bind(new IPEndPoint(IPAddress.Loopback, 0));
        return ((IPEndPoint)s.LocalEndPoint!).Port;
    }

    /// Real `netstat -ano -p UDP` shape, with the LE-87 collision in it: PIDs
    /// 23988 and 12044 both on UDP :69.
    private const string NetstatSample = """

        Active Connections

          Proto  Local Address          Foreign Address        State
          UDP    0.0.0.0:67             *:*                                    4242
          UDP    0.0.0.0:69             *:*                                    23988
          UDP    0.0.0.0:69             *:*                                    12044
          UDP    0.0.0.0:690            *:*                                    5150
          UDP    0.0.0.0:6900           *:*                                    5151
          UDP    [::]:67                *:*                                    4242
          TCP    0.0.0.0:69             0.0.0.0:0              LISTENING        7777
        """;
}
