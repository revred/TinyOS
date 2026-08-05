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
//   tos64-netboot --mac 88:a2:9e:11:4e:cc --root <dir> [--server <ip>] [--log-only]
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
        _serverIp = server is not null ? IPAddress.Parse(server) : GuessLinkLocalAddress();

        Console.WriteLine("tos64-netboot — Pi 5 firmware netboot, charter-neutral by construction");
        Console.WriteLine($"  answering ONLY   : {mac}");
        Console.WriteLine($"  server address   : {_serverIp}");
        Console.WriteLine($"  offering client  : {_offeredIp}");
        Console.WriteLine($"  tftp root        : {(_logOnly ? "(log-only: no file will be served)" : Path.GetFullPath(_root))}");
        Console.WriteLine();
        Console.WriteLine("  Ctrl-C to stop. Reboot the board now.");
        Console.WriteLine();

        var tftp = new Thread(TftpLoop) { IsBackground = true };
        tftp.Start();
        DhcpLoop();
        return 0;
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

    /// The laptop's own address on the bench link.
    ///
    /// Link-local (169.254/16) by preference, because that is what a
    /// point-to-point cable with no DHCP server produces on both ends, and it
    /// is what the Pi is already using.
    private static IPAddress GuessLinkLocalAddress()
    {
        foreach (var nic in NetworkInterface.GetAllNetworkInterfaces())
        {
            if (nic.OperationalStatus != OperationalStatus.Up) continue;
            foreach (var a in nic.GetIPProperties().UnicastAddresses)
            {
                if (a.Address.AddressFamily != AddressFamily.InterNetwork) continue;
                var b = a.Address.GetAddressBytes();
                if (b[0] == 169 && b[1] == 254) return a.Address;
            }
        }
        return IPAddress.Any;
    }

    // ---- DHCP -------------------------------------------------------------

    private static void DhcpLoop()
    {
        using var socket = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
        socket.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.Broadcast, true);
        socket.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.ReuseAddress, true);
        try
        {
            socket.Bind(new IPEndPoint(IPAddress.Any, DhcpServerPort));
        }
        catch (SocketException e)
        {
            Console.Error.WriteLine($"tos64-netboot: cannot bind UDP {DhcpServerPort}: {e.Message}");
            Console.Error.WriteLine("  Another DHCP server is running, or this needs elevation.");
            return;
        }

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
            var reply = BuildReply(buffer, messageType == 1 ? (byte)2 : (byte)5);
            socket.SendTo(reply, new IPEndPoint(IPAddress.Broadcast, DhcpClientPort));
            Console.WriteLine($"  -> sent {(messageType == 1 ? "OFFER" : "ACK")}: " +
                              $"yiaddr={_offeredIp} siaddr={_serverIp} file=\"{BootFileName}\" +opt43");
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
    private static void TftpLoop()
    {
        using var socket = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
        socket.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.ReuseAddress, true);
        try
        {
            socket.Bind(new IPEndPoint(IPAddress.Any, TftpPort));
        }
        catch (SocketException e)
        {
            Console.Error.WriteLine($"tos64-netboot: cannot bind UDP {TftpPort}: {e.Message}");
            return;
        }

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

            var path = Path.GetFullPath(Path.Combine(_root, name.Replace('/', Path.DirectorySeparatorChar)));
            var rootFull = Path.GetFullPath(_root);
            // Refuse anything that escapes the root. A path traversal in a
            // bench tool is still a path traversal.
            if (!path.StartsWith(rootFull, StringComparison.OrdinalIgnoreCase))
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
            Console.WriteLine($"  -> serving {bytes.Length} bytes");
            Serve(socket, from, bytes);
        }
    }

    /// Classic 512-byte-block TFTP, no options, no windowing. Bounded retries
    /// so a client that stops answering ends the transfer instead of pinning a
    /// thread forever — fail-safe over keep-trying, the same rule the board
    /// holds itself to.
    private static void Serve(Socket socket, EndPoint client, byte[] data)
    {
        const int Block = 512;
        var total = (data.Length / Block) + 1;
        for (var index = 1; index <= total; index++)
        {
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
