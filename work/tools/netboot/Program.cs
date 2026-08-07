// tos64-netboot — the thing that ends the card swap.
//
// A deliberately tiny DHCP responder and TFTP server, for exactly one client:
// the Raspberry Pi 5 bootloader, before TinyOS exists.
//
// WHY THIS IS CHARTER-NEUTRAL, which is the whole reason it is allowed to exist
// at all. The Pi 5 firmware fetches kernel8.img over TFTP and jumps to it
// BEFORE any TinyOS code runs. TinyOS never receives the image, never parses
// it, and never admits it — so agent.md rule 9 ("remote bytes are data, never
// code") is not engaged, gem.rs keeps its `no_path_in_this_module_ever_enables_
// receive` guarantee, and LE-67's containment story is untouched. The
// alternative — TinyOS receiving an image at runtime and executing it — would
// need all fourteen RCG-* gates and is a different project.
//
// THE ONE RULE THIS PROGRAM WILL NOT BEND: it answers exactly one MAC address,
// given on the command line, and ignores every other DHCP client on the wire.
// A DHCP responder that answers broadcasts indiscriminately is how a bench tool
// takes down a household. The bench link is point-to-point today (the Pi holds
// only a link-local address and sees exactly one neighbour), but "there is
// nothing else on this cable" is a fact about today, not a property of the
// program, so the restriction lives in the code.
//
// It is also READ-ONLY: TFTP write requests are refused, not ignored, so a
// misconfigured client gets an error rather than silence.
//
// Usage:
//   tos64-netboot --mac 88:a2:9e:11:4e:cc --root <dir> [--server <ip>] [--offer <ip>]
//                 [--log-only]
//
//   --offer      the address handed to the board in the DHCP OFFER, when it must
//                differ from --server. Accepted by the parser since this tool was
//                written and omitted from this comment until 2026-08-07, which is
//                the shape LE-80 is about: the documentation and the code came
//                apart and only the code was right.
//
//   --log-only   answer DHCP but serve no files, so a first run records what
//                the bootloader actually asks for without anything having to be
//                correct yet. This is netboot investigation question 3, which
//                cannot be observed any other way: without a DHCP offer the
//                bootloader never reaches TFTP at all.

using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Text;

internal static class Program
{
    private const int DhcpServerPort = 67;
    private const int DhcpClientPort = 68;
    private const int TftpPort = 69;

    private static byte[] _clientMac = [];
    private static IPAddress _serverIp = IPAddress.Any;
    private static IPAddress _offeredIp = IPAddress.Parse("169.254.133.66");
    private static string _root = ".";
    private static bool _logOnly;

    /// The file named in the offer.
    ///
    /// The Pi 5 bootloader has its own convention for what it fetches, so this
    /// is a PROMPT rather than an instruction: naming something gets the client
    /// as far as TFTP, and the request log then records what it actually asks
    /// for - which is the observation the whole first run exists to make. An
    /// empty name left it re-discovering with nothing to request.
    private const string BootFileName = "bootcode.bin";

    private static int Main(string[] args)
    {
        Unbuffer();

        string? mac = null;
        string? server = null;
        for (var i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--mac" when i + 1 < args.Length: mac = args[++i]; break;
                case "--root" when i + 1 < args.Length: _root = args[++i]; break;
                case "--server" when i + 1 < args.Length: server = args[++i]; break;
                case "--offer" when i + 1 < args.Length:
                    _offeredIp = IPAddress.Parse(args[++i]); break;
                case "--log-only": _logOnly = true; break;
                case "-h" or "--help": Usage(); return 0;
            }
        }

        if (mac is null)
        {
            Console.Error.WriteLine("tos64-netboot: --mac is required and is not optional.");
            Console.Error.WriteLine("  Answering every DHCP client on a wire is how a bench tool");
            Console.Error.WriteLine("  takes down a household. Name the board you mean.");
            Usage();
            return 2;
        }

        _clientMac = ParseMac(mac);

        // `LE-97`: A SERVER THAT CANNOT NAME ITS OWN ADDRESS DOES NOT START.
        //
        // This used to fall back to `IPAddress.Any`, and on 2026-08-06 it did:
        // the bench NIC is `Disconnected` until the board powers, so discovery
        // found nothing, `0.0.0.0` went into the DHCP OFFER, and the board would
        // have been handed `siaddr=0.0.0.0`, failed to fetch, and looked like a
        // board fault. It printed `0.0.0.0` in the same confident column it
        // prints a real address, and only a human reading that line stood
        // between it and a wasted power cycle diagnosed as hardware.
        //
        // Printing the value it chose is not the same as refusing a value it
        // cannot justify — which is why the fix is a refusal and not a better
        // guess. Ambiguity is refused too: this host holds four link-local
        // addresses and first-one-wins fails the same way with a PLAUSIBLE
        // address in the log, which is strictly harder to catch.
        var host = ServerAddress.EnumerateHost();
        var choice = ServerAddress.Choose(
            server,
            ServerAddress.LinkLocalCandidates(host),
            ServerAddress.KnownHostAddresses(host));
        if (!choice.CanServe)
        {
            Console.Error.WriteLine("tos64-netboot: REFUSING TO START — cannot name my own address.");
            Console.Error.WriteLine($"  {choice.Explain()}");
            foreach (var candidate in choice.Candidates)
            {
                Console.Error.WriteLine($"    candidate: {candidate}");
            }
            return 4;
        }
        _serverIp = choice.Address!;

        Console.WriteLine("tos64-netboot — Pi 5 firmware netboot, charter-neutral by construction");
        Console.WriteLine($"  answering ONLY   : {mac}");
        Console.WriteLine($"  server address   : {_serverIp}");
        // Printed on EVERY path, refusal or not: the tool saying which of the
        // six decisions it made is the difference between a line an operator
        // can check and a line they can only read.
        Console.WriteLine($"    {choice.Explain()}");
        if (choice.IsWarning)
        {
            // Loud, and on stderr, because this is the last way LE-97 can still
            // happen: syntax was checked and existence was not.
            Console.Error.WriteLine();
            Console.Error.WriteLine("  !! WARNING — the named address is not one this host appears to hold.");
            Console.Error.WriteLine($"  !! {choice.Explain()}");
            Console.Error.WriteLine("  !! Serving anyway: refusing here would refuse the case where the");
            Console.Error.WriteLine("  !! board is not powered yet, which is the case that has to work.");
            Console.Error.WriteLine();
        }
        Console.WriteLine($"  offering client  : {_offeredIp}");
        Console.WriteLine($"  tftp root        : {(_logOnly ? "(log-only: no file will be served)" : Path.GetFullPath(_root))}");
        Console.WriteLine();

        // BOTH ports are claimed before EITHER loop starts, and a port already
        // held ends the run here (LE-87). Binding inside each loop is how the
        // tool came to answer DHCP correctly while a stale instance served
        // TFTP: one half succeeded, the other half was never reached, and
        // every visible signal said the run was good.
        var dhcpSocket = Claim(DhcpServerPort, "DHCP");
        if (dhcpSocket is null) return 3;
        var tftpSocket = Claim(TftpPort, "TFTP");
        if (tftpSocket is null) { dhcpSocket.Dispose(); return 3; }

        Console.WriteLine($"  ports held       : UDP {DhcpServerPort} (DHCP) + UDP {TftpPort} (TFTP), exclusively");
        Console.WriteLine();
        Console.WriteLine("  Ctrl-C to stop. Reboot the board now.");
        Console.WriteLine();

        var tftp = new Thread(() => TftpLoop(tftpSocket)) { IsBackground = true };
        tftp.Start();
        DhcpLoop(dhcpSocket);
        return 0;
    }

    /// Claims one UDP port for this process alone, or explains who has it.
    ///
    /// The bind is the decision; `netstat` is only diagnosis, so a bind that
    /// fails for some other reason still stops the run and still says what the
    /// operating system said.
    private static Socket? Claim(int port, string role)
    {
        try
        {
            return PortGuard.BindExclusive(IPAddress.Any, port);
        }
        catch (SocketException e)
        {
            Console.Error.WriteLine($"tos64-netboot: REFUSING TO START — cannot take UDP {port} ({role}): {e.SocketErrorCode}");
            var holders = PortGuard.HoldersOf(port);
            if (holders.Count > 0)
            {
                Console.Error.WriteLine($"  UDP {port} is held by {holders.Count} process(es):");
                foreach (var line in PortGuard.Describe(holders)) Console.Error.WriteLine(line);
                Console.Error.WriteLine("  Stop it and start again. A second instance would not have failed");
                Console.Error.WriteLine("  loudly here — it would have shared the port and served a stale");
                Console.Error.WriteLine("  image while this one logged a clean DHCP exchange (LE-87).");
            }
            else if (e.SocketErrorCode == SocketError.AccessDenied)
            {
                Console.Error.WriteLine("  Access denied and no holder found: this needs an elevated shell.");
            }
            else
            {
                Console.Error.WriteLine("  No holder could be identified; netstat was unreadable or the port");
                Console.Error.WriteLine("  is held by something it does not list. The bind is authoritative.");
            }
            return null;
        }
    }

    /// Makes the log reach the file while the server is still running.
    ///
    /// Redirected stdout is BUFFERED — 4 KiB, flushed on exit. A bench session
    /// redirects this tool to a log and reads it in another window, so every
    /// line after the first few sat invisible until the process was killed:
    /// the DHCP exchange, the TFTP requests, and the served file's digest, all
    /// of which exist to be read WHILE the board is booting. Found 2026-08-06
    /// verifying the LE-87 fix, and it is the same family as the defect it was
    /// found verifying — the tool did the work and the report did not arrive.
    private static void Unbuffer()
    {
        Console.SetOut(new StreamWriter(Console.OpenStandardOutput()) { AutoFlush = true });
        Console.SetError(new StreamWriter(Console.OpenStandardError()) { AutoFlush = true });
    }

    private static void Usage()
    {
        Console.WriteLine();
        Console.WriteLine("  tos64-netboot --mac <aa:bb:cc:dd:ee:ff> --root <dir> [--server <ip>]");
        Console.WriteLine("                [--offer <ip>] [--log-only]");
        Console.WriteLine();
        Console.WriteLine("  --log-only answers DHCP but serves nothing, so a first run records");
        Console.WriteLine("  what the bootloader asks for before anything has to be correct.");
    }

    private static byte[] ParseMac(string text)
    {
        var parts = text.Split(':', '-');
        if (parts.Length != 6) throw new ArgumentException($"not a MAC address: {text}");
        return parts.Select(p => Convert.ToByte(p, 16)).ToArray();
    }

    // The laptop's own address on the bench link is decided by
    // `ServerAddress.Choose`, which is pure and tested. `GuessLinkLocalAddress`
    // lived here and returned the FIRST match with `IPAddress.Any` as its
    // fallback; both halves of that were defects (`LE-97`).

    // ---- DHCP -------------------------------------------------------------

    /// Answers the one board named on the command line, on a socket already
    /// bound exclusively by `Claim`.
    private static void DhcpLoop(Socket socket)
    {
        using var owned = socket;
        socket.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.Broadcast, true);

        var buffer = new byte[2048];
        while (true)
        {
            EndPoint from = new IPEndPoint(IPAddress.Any, 0);
            int n;
            try { n = socket.ReceiveFrom(buffer, ref from); }
            catch (SocketException) { continue; }
            if (n < 240) continue;

            // BOOTP: op(1) htype(1) hlen(1) hops(1) xid(4) secs(2) flags(2)
            //        ciaddr(4) yiaddr(4) siaddr(4) giaddr(4) chaddr(16) ...
            if (buffer[0] != 1) continue;                       // requests only
            var chaddr = buffer[28..34];
            if (!chaddr.SequenceEqual(_clientMac))
            {
                // The rule this program will not bend. Named, not silent, so a
                // wrong --mac is visible rather than looking like a dead wire.
                Console.WriteLine($"  ignoring DHCP from {Mac(chaddr)} (not the board we were told to answer)");
                continue;
            }

            var (messageType, vendorClass) = ReadOptions(buffer, n);
            Console.WriteLine($"DHCP {Describe(messageType)} from {Mac(chaddr)}" +
                              (vendorClass is null ? "" : $"  vendor-class=\"{vendorClass}\""));

            if (messageType is not (1 or 3)) continue;          // DISCOVER, REQUEST

            // The deferred re-guess that used to live here is GONE (`LE-97`).
            // It existed because startup could produce `IPAddress.Any` and this
            // is the first moment the link is provably live — but it was a
            // second mechanism for the same decision, and the one that fired
            // late could still pick the wrong one of four link-local addresses.
            // `Main` now refuses to start without a single justified address, so
            // `_serverIp` is never the wildcard by the time a DISCOVER arrives
            // and there is exactly one place that decides it.

            var reply = BuildReply(buffer, messageType == 1 ? (byte)2 : (byte)5);

            // Fail-safe over keep-trying's opposite failure: an unhandled
            // SocketException here killed the whole server on the FIRST packet
            // it ever answered (10065, no route for 255.255.255.255 from a
            // socket bound to 0.0.0.0 across several link-local NICs). A send
            // that fails must cost this one reply, never the session — the
            // bootloader retries DISCOVER, and a live server can answer the
            // retry. Sent from a socket bound to the bench address so Windows
            // cannot pick the Bluetooth or WiFi route for the broadcast.
            if (!TrySendBroadcast(reply))
            {
                continue;
            }

            Console.WriteLine($"  -> sent {(messageType == 1 ? "OFFER" : "ACK")}: " +
                              $"yiaddr={_offeredIp} siaddr={_serverIp} file=\"{BootFileName}\" +opt43");
        }
    }

    /// Broadcasts one DHCP reply out the bench interface, reporting rather than
    /// throwing.
    ///
    /// Bound to `_serverIp` deliberately: this host holds five IPv4 addresses,
    /// four of them link-local (Bluetooth, two WiFi-Direct virtuals, the bench
    /// Ethernet), and an unbound broadcast lets the stack choose among them.
    /// The one that matters is the one the board is cabled to.
    private static bool TrySendBroadcast(byte[] reply)
    {
        try
        {
            using var send = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            send.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.Broadcast, true);
            send.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.ReuseAddress, true);
            send.Bind(new IPEndPoint(_serverIp, 0));
            send.SendTo(reply, new IPEndPoint(IPAddress.Broadcast, DhcpClientPort));
            return true;
        }
        catch (SocketException e)
        {
            Console.Error.WriteLine($"  !! reply not sent ({e.SocketErrorCode}): {e.Message}");
            Console.Error.WriteLine($"     bound to {_serverIp}; the bootloader will retry DISCOVER.");
            return false;
        }
    }

    private static string Describe(byte type) => type switch
    {
        1 => "DISCOVER", 2 => "OFFER", 3 => "REQUEST", 4 => "DECLINE",
        5 => "ACK", 6 => "NAK", 7 => "RELEASE", 8 => "INFORM",
        _ => $"type-{type}",
    };

    private static (byte Type, string? VendorClass) ReadOptions(byte[] p, int len)
    {
        byte type = 0;
        string? vendor = null;
        var at = 240;                                          // after the magic cookie
        while (at + 1 < len)
        {
            var code = p[at];
            if (code == 255) break;                            // END
            if (code == 0) { at++; continue; }                 // PAD
            var size = p[at + 1];
            if (at + 2 + size > len) break;
            if (code == 53 && size >= 1) type = p[at + 2];
            if (code == 60) vendor = Encoding.ASCII.GetString(p, at + 2, size);
            at += 2 + size;
        }
        return (type, vendor);
    }

    /// A BOOTP reply carrying the two fields the firmware actually needs:
    /// `siaddr` (where to TFTP from) and `file` (what to ask for).
    ///
    /// `file` is left EMPTY on purpose. The Pi 5 bootloader has its own
    /// convention for what it fetches and from which directory, and inventing a
    /// filename here would be guessing at the very thing the first run exists to
    /// observe — the design-before-ground-truth mistake this bench keeps
    /// learning not to make. The TFTP side logs every request verbatim.
    private static byte[] BuildReply(byte[] request, byte messageType)
    {
        // 548 is the minimum DHCP message size a client must accept; the option
        // block outgrew a 300-byte buffer the moment PXE option 43 was added,
        // which is worth a constant rather than another silent near-miss.
        var r = new byte[548];
        r[0] = 2;                                              // BOOTREPLY
        r[1] = request[1];                                     // htype
        r[2] = request[2];                                     // hlen
        Array.Copy(request, 4, r, 4, 4);                       // xid
        Array.Copy(request, 10, r, 10, 2);                     // flags
        _offeredIp.GetAddressBytes().CopyTo(r, 16);            // yiaddr
        _serverIp.GetAddressBytes().CopyTo(r, 20);             // siaddr
        Array.Copy(request, 28, r, 28, 16);                    // chaddr

        // sname (44..108) and file (108..236): the BOOTP header fields, filled
        // as well as the equivalent options, because clients differ in which
        // they read and the cost of stating both is zero.
        var sname = Encoding.ASCII.GetBytes(_serverIp.ToString());
        Array.Copy(sname, 0, r, 44, Math.Min(sname.Length, 63));
        var file = Encoding.ASCII.GetBytes(BootFileName);
        Array.Copy(file, 0, r, 108, Math.Min(file.Length, 127));

        var at = 236;
        r[at++] = 99; r[at++] = 130; r[at++] = 83; r[at++] = 99;   // magic cookie
        r[at++] = 53; r[at++] = 1; r[at++] = messageType;          // message type
        r[at++] = 54; r[at++] = 4;                                  // server id
        _serverIp.GetAddressBytes().CopyTo(r, at); at += 4;
        r[at++] = 1; r[at++] = 4;                                   // subnet mask
        new byte[] { 255, 255, 0, 0 }.CopyTo(r, at); at += 4;
        r[at++] = 51; r[at++] = 4;                                  // lease time
        new byte[] { 0, 0, 0x0E, 0x10 }.CopyTo(r, at); at += 4;     // 1 hour
        // Vendor class, echoed: the client announces PXEClient and will ignore
        // an offer that does not answer in kind.
        var pxe = Encoding.ASCII.GetBytes("PXEClient");
        r[at++] = 60; r[at++] = (byte)pxe.Length;
        pxe.CopyTo(r, at); at += pxe.Length;

        // Option 43, PXE vendor sub-options. WITHOUT THIS THE BOARD REJECTS THE
        // OFFER AND RE-DISCOVERS FOREVER - observed on the bench: two DISCOVERs
        // answered with two OFFERs and never a REQUEST. A PXE client that has
        // announced itself as one expects boot-server guidance, and an offer
        // carrying an address but no such guidance is not an offer it can use.
        //
        // Sub-option 6 (discovery control) = 3 means "do not multicast or
        // broadcast to find a boot server; use the file name you were given",
        // which collapses the whole PXE boot-server negotiation into the two
        // fields already set above. It is the smallest thing that can work.
        r[at++] = 43; r[at++] = 4;
        r[at++] = 6; r[at++] = 1; r[at++] = 3;                      // discovery control
        r[at++] = 255;                                              // end of sub-options

        // Option 66/67: the TFTP server and the file, stated explicitly as well
        // as in siaddr/file, because clients differ in which they read.
        var tftpServer = Encoding.ASCII.GetBytes(_serverIp.ToString());
        r[at++] = 66; r[at++] = (byte)tftpServer.Length;
        tftpServer.CopyTo(r, at); at += tftpServer.Length;
        var bootfile = Encoding.ASCII.GetBytes(BootFileName);
        r[at++] = 67; r[at++] = (byte)bootfile.Length;
        bootfile.CopyTo(r, at); at += bootfile.Length;

        r[at++] = 255;                                              // END
        return r[..Math.Max(at, 300)];
    }

    private static string Mac(byte[] m) => string.Join(':', m.Select(b => b.ToString("x2")));

    // ---- TFTP -------------------------------------------------------------

    /// Read-only TFTP. Every request is logged verbatim BEFORE any decision
    /// about whether the file exists, because the log is the point of the first
    /// run: it is the only way to observe what the firmware asks for.
    private static void TftpLoop(Socket socket)
    {
        using var owned = socket;

        var buffer = new byte[1024];
        while (true)
        {
            EndPoint from = new IPEndPoint(IPAddress.Any, 0);
            int n;
            try { n = socket.ReceiveFrom(buffer, ref from); }
            catch (SocketException) { continue; }
            if (n < 4) continue;

            var opcode = (buffer[0] << 8) | buffer[1];
            var fields = Encoding.ASCII.GetString(buffer, 2, n - 2).Split('\0');
            var name = fields.Length > 0 ? fields[0] : "";

            if (opcode == 2)
            {
                Console.WriteLine($"TFTP WRITE refused: \"{name}\" — this server is read-only");
                SendError(socket, from, 2, "read-only server");
                continue;
            }
            if (opcode != 1)
            {
                continue;                                      // DATA/ACK/ERROR: not ours to start
            }

            Console.WriteLine($"TFTP RRQ  \"{name}\"" +
                              (fields.Length > 1 ? $"  mode={fields[1]}" : ""));

            if (_logOnly)
            {
                SendError(socket, from, 1, "log-only run: file not served");
                continue;
            }

            // Name to file, or refusal, in one decision — see `TftpPaths` for
            // why the leading-slash spelling of the root is not an escape, and
            // for what it cost on 2026-08-06 when it was treated as one.
            var path = TftpPaths.Resolve(_root, name);
            if (path is null)
            {
                Console.WriteLine($"  -> REFUSED: outside the tftp root");
                SendError(socket, from, 2, "access violation");
                continue;
            }
            if (!File.Exists(path))
            {
                Console.WriteLine($"  -> not found");
                SendError(socket, from, 1, "file not found");
                continue;
            }

            var bytes = File.ReadAllBytes(path);
            // Digest and absolute path on every transfer: what was served has
            // to be comparable against what was built without a power cycle to
            // find out (LE-87).
            Console.WriteLine(TransferLog.Served(path, bytes));
            // The marker that stops `tos64-power` cutting mains mid-image
            // (`LE-95` clause 2). Cleared in a `finally`, because a marker left
            // behind by a crash would block every later power cycle — the guard
            // reads an old one as `Stale` and proceeds, but only because THIS
            // side promises to clear it on the ordinary path.
            try
            {
                Serve(socket, from, bytes, name);
            }
            finally
            {
                TransferBeacon.Clear(_root);
            }
        }
    }

    /// Classic 512-byte-block TFTP, no options, no windowing. Bounded retries
    /// so a client that stops answering ends the transfer instead of pinning a
    /// thread forever — fail-safe over keep-trying, the same rule the board
    /// holds itself to.
    private static void Serve(Socket socket, EndPoint client, byte[] data, string name)
    {
        const int Block = 512;
        var total = (data.Length / Block) + 1;
        for (var index = 1; index <= total; index++)
        {
            // Refreshed per block rather than stamped once at the start: the
            // marker has to say "still progressing", not "began", or a slow
            // transfer reads as stale after ten seconds and `tos64-power`
            // cycles mains straight through the middle of it.
            TransferBeacon.Mark(_root, name);

            var offset = (index - 1) * Block;
            var size = Math.Min(Block, data.Length - offset);
            var packet = new byte[4 + size];
            packet[0] = 0; packet[1] = 3;
            packet[2] = (byte)(index >> 8); packet[3] = (byte)index;
            Array.Copy(data, offset, packet, 4, size);

            var acked = false;
            for (var attempt = 0; attempt < 5 && !acked; attempt++)
            {
                socket.SendTo(packet, client);
                socket.ReceiveTimeout = 2000;
                var ack = new byte[64];
                try
                {
                    EndPoint from = new IPEndPoint(IPAddress.Any, 0);
                    var n = socket.ReceiveFrom(ack, ref from);
                    if (n >= 4 && ack[0] == 0 && ack[1] == 4 &&
                        ((ack[2] << 8) | ack[3]) == (index & 0xFFFF))
                    {
                        acked = true;
                    }
                }
                catch (SocketException) { /* retry */ }
            }
            if (!acked)
            {
                Console.WriteLine($"  -> transfer abandoned at block {index}/{total} (client stopped acking)");
                return;
            }
        }
        socket.ReceiveTimeout = 0;
        Console.WriteLine($"  -> transfer complete ({total} block(s))");
    }

    private static void SendError(Socket socket, EndPoint to, ushort code, string message)
    {
        var text = Encoding.ASCII.GetBytes(message);
        var packet = new byte[4 + text.Length + 1];
        packet[0] = 0; packet[1] = 5;
        packet[2] = (byte)(code >> 8); packet[3] = (byte)code;
        text.CopyTo(packet, 4);
        socket.SendTo(packet, to);
    }
}
