using System.Diagnostics;
using System.Net;
using System.Net.Sockets;

/// Refusing to start when someone else already holds the port.
///
/// LE-87, 2026-08-06: an instance left running from the previous session
/// (started 05/08 22:00) and a fresh one (00:52) both held UDP 69. The new one
/// answered DHCP — OFFER and ACK both logged, both correct — and the OLD one
/// received every TFTP RRQ and served ITS root. The board netbooted a stale
/// kernel8.img and emitted a complete, plausible, entirely wrong envelope. It
/// was caught only because a metric was missing BY NAME rather than merely
/// different by value; three power cycles were spent first.
///
/// The mechanism was SO_REUSEADDR on both sockets, so the second bind
/// succeeded silently and Windows delivered each datagram to one of the two.
/// A bench server that silently shares its port is worse than one that will
/// not start.
internal static class PortGuard
{
    /// Binds a UDP port with no address reuse, so a port already held produces
    /// a `SocketException` rather than a second quiet listener.
    ///
    /// The socket is bound HERE, before any thread starts, and handed to the
    /// loop that will use it. Probing and then binding later would leave a
    /// window in which the answer is stale — and this is a guard against
    /// exactly the class of bug where two things that should be one are not.
    internal static Socket BindExclusive(IPAddress address, int port)
    {
        var socket = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
        try
        {
            socket.Bind(new IPEndPoint(address, port));
        }
        catch
        {
            socket.Dispose();
            throw;
        }
        return socket;
    }

    /// Every process holding a UDP port, per `netstat -ano -p UDP`.
    ///
    /// `netstat` and not `Get-NetUDPEndpoint`: on the run that produced LE-87,
    /// netstat showed two PIDs on :69 and `Get-NetUDPEndpoint` reported one,
    /// which is why the first look did not find the collision. Diagnosis is
    /// best-effort — an empty list here means "could not tell", never "nobody",
    /// and the bind is what actually decides.
    internal static IReadOnlyList<int> HoldersOf(int port)
    {
        try
        {
            using var netstat = Process.Start(new ProcessStartInfo("netstat", "-ano -p UDP")
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            });
            if (netstat is null) return [];
            var output = netstat.StandardOutput.ReadToEnd();
            netstat.WaitForExit(5000);
            return ParseUdpHolders(output, port);
        }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException)
        {
            return [];
        }
    }

    /// The PIDs on one UDP port, distinct, from `netstat -ano` output.
    ///
    /// Rows look like `  UDP    0.0.0.0:69   *:*   23988`, and the IPv6 form
    /// spells the address `[::]:69`, so the port is what follows the LAST
    /// colon. Matched whole: `:690` and `:6900` are different ports, and TCP
    /// rows hold nothing this tool binds.
    internal static IReadOnlyList<int> ParseUdpHolders(string netstatOutput, int port)
    {
        var found = new List<int>();
        foreach (var raw in netstatOutput.Split('\n'))
        {
            var fields = raw.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
            if (fields.Length < 4) continue;
            if (!fields[0].Equals("UDP", StringComparison.OrdinalIgnoreCase)) continue;

            var local = fields[1];
            var colon = local.LastIndexOf(':');
            if (colon < 0) continue;
            if (!int.TryParse(local.AsSpan(colon + 1), out var boundPort) || boundPort != port) continue;

            if (!int.TryParse(fields[^1], out var pid)) continue;
            if (!found.Contains(pid)) found.Add(pid);
        }
        return found;
    }

    /// One line per holder, naming the process as well as the PID, because
    /// "another tos64-netboot from the last session" and "the Windows Deployment
    /// Services you forgot you installed" call for different actions.
    internal static IEnumerable<string> Describe(IReadOnlyList<int> pids)
    {
        foreach (var pid in pids)
        {
            string name;
            try { name = Process.GetProcessById(pid).ProcessName; }
            catch (ArgumentException) { name = "(exited since netstat ran)"; }
            catch (InvalidOperationException) { name = "(unreadable)"; }
            yield return $"    pid {pid}  {name}";
        }
    }
}
