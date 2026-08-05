// Live capture for Ti64Dink — the thin slice of libpcap we actually need.
//
// Taken as inspiration from external/npcap188 rather than as a dependency on
// it: four entry points, P/Invoked straight into wpcap.dll, so this tool keeps
// zero package references and builds on a bench with no internet, exactly like
// tos64-cardswap and tos64-linkwatch.
//
// Why a driver is involved at all: Windows demultiplexes inbound frames by
// EtherType inside NDIS, in the kernel, and discards anything with no
// registered protocol driver. EtherType 0x88B5 has none, so the frames never
// reach user mode at any privilege level — there is no Winsock or WFP path to
// them. Npcap's NDIS filter driver is what puts them within reach; this file
// is only the client of it.
//
// Filtering is done here in managed code rather than through pcap_compile /
// pcap_setfilter. Two fewer P/Invokes, no BPF program lifetime to manage, and
// the check is one 16-bit comparison against a constant — the cost is
// irrelevant beside the fact that the decode path stays identical to the
// file-based one.

using System.Runtime.InteropServices;

internal static class Live
{
    private const string Wpcap = "wpcap.dll";
    private const int ErrbufSize = 256;

    /// The board's EtherType: IEEE 802 local experimental (FEAT-P1-09/-10).
    private const ushort Tos64EtherType = 0x88B5;

    [StructLayout(LayoutKind.Sequential)]
    private struct PcapIf
    {
        public IntPtr Next;
        public IntPtr Name;
        public IntPtr Description;
        public IntPtr Addresses;
        public uint Flags;
    }

    // `long` is 32-bit on Windows, so timeval is two int32s — getting this
    // wrong silently shifts caplen/len and every frame reads as garbage.
    [StructLayout(LayoutKind.Sequential)]
    private struct PcapPktHdr
    {
        public int TvSec;
        public int TvUsec;
        public uint CapLen;
        public uint Len;
    }

    [DllImport(Wpcap, CallingConvention = CallingConvention.Cdecl)]
    private static extern int pcap_findalldevs(ref IntPtr alldevs, byte[] errbuf);

    [DllImport(Wpcap, CallingConvention = CallingConvention.Cdecl)]
    private static extern void pcap_freealldevs(IntPtr alldevs);

    [DllImport(Wpcap, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr pcap_open_live(
        [MarshalAs(UnmanagedType.LPStr)] string device,
        int snaplen, int promisc, int toMs, byte[] errbuf);

    [DllImport(Wpcap, CallingConvention = CallingConvention.Cdecl)]
    private static extern int pcap_next_ex(IntPtr handle, ref IntPtr header, ref IntPtr data);

    [DllImport(Wpcap, CallingConvention = CallingConvention.Cdecl)]
    private static extern void pcap_close(IntPtr handle);

    internal sealed record Device(string Name, string Description);

    internal static List<Device> Devices()
    {
        var devices = new List<Device>();
        var errbuf = new byte[ErrbufSize];
        var head = IntPtr.Zero;
        if (pcap_findalldevs(ref head, errbuf) != 0)
        {
            throw new InvalidOperationException("pcap_findalldevs: " + Str(errbuf));
        }
        try
        {
            for (var p = head; p != IntPtr.Zero;)
            {
                var iface = Marshal.PtrToStructure<PcapIf>(p);
                devices.Add(new Device(
                    Marshal.PtrToStringAnsi(iface.Name) ?? "",
                    Marshal.PtrToStringAnsi(iface.Description) ?? ""));
                p = iface.Next;
            }
        }
        finally
        {
            if (head != IntPtr.Zero) pcap_freealldevs(head);
        }
        return devices;
    }

    /// Captures for `seconds`, returning the payload of every TOS64 frame seen.
    ///
    /// Payloads only — the 14-byte Ethernet header is stripped here so the
    /// decoder receives exactly what it receives from a file, and cannot come
    /// to depend on the capture source.
    internal static List<byte[]> Capture(string device, int seconds, out int framesSeen) =>
        Capture(device, seconds, sighted: null, out framesSeen);

    /// Captures EVERY frame, not just TOS64 ones, returning each WITH its
    /// 14-byte Ethernet header intact.
    ///
    /// For watching what the Pi 5 bootloader does before TinyOS exists: its
    /// netboot traffic is DHCP and TFTP over IPv4/UDP (EtherType 0x0800), so
    /// the 0x88B5 filter that makes every other capture clean is precisely
    /// what blinds this one. The header is kept because here the addresses and
    /// the EtherType ARE the evidence - which MAC asked, and for what.
    internal static List<byte[]> CaptureAny(string device, int seconds, out int framesSeen)
    {
        var errbuf = new byte[ErrbufSize];
        var handle = pcap_open_live(device, 65536, 1, 100, errbuf);
        if (handle == IntPtr.Zero)
        {
            throw new InvalidOperationException("pcap_open_live: " + Str(errbuf));
        }

        var frames = new List<byte[]>();
        framesSeen = 0;
        var deadline = DateTime.UtcNow.AddSeconds(seconds);
        try
        {
            while (DateTime.UtcNow < deadline)
            {
                IntPtr headerPtr = IntPtr.Zero, dataPtr = IntPtr.Zero;
                var rc = pcap_next_ex(handle, ref headerPtr, ref dataPtr);
                if (rc == 0) continue;
                if (rc < 0) break;
                if (headerPtr == IntPtr.Zero || dataPtr == IntPtr.Zero) continue;

                var header = Marshal.PtrToStructure<PcapPktHdr>(headerPtr);
                var caplen = (int)header.CapLen;
                if (caplen < 14) continue;

                var frame = new byte[caplen];
                Marshal.Copy(dataPtr, frame, 0, caplen);
                framesSeen++;
                frames.Add(frame);
            }
        }
        finally
        {
            pcap_close(handle);
        }
        return frames;
    }

    /// The `--until` shape: same capture, but each payload is offered to
    /// `sighted` as it arrives and the capture ends EARLY the moment the
    /// predicate returns true. The deadline still stands — a condition that
    /// never happens must end as a reported timeout, never as a hung bench
    /// (fail-safe over keep-trying; the caller reads which of the two
    /// happened from whether the predicate ever fired, not from elapsed time).
    internal static List<byte[]> Capture(
        string device, int seconds, Func<byte[], bool>? sighted, out int framesSeen)
    {
        var errbuf = new byte[ErrbufSize];
        // 65536 snaplen so nothing is ever truncated; promiscuous because the
        // board broadcasts and we are not its addressee; 100 ms read timeout so
        // a quiet wire still returns control and the deadline is honoured.
        var handle = pcap_open_live(device, 65536, 1, 100, errbuf);
        if (handle == IntPtr.Zero)
        {
            throw new InvalidOperationException("pcap_open_live: " + Str(errbuf));
        }

        var payloads = new List<byte[]>();
        framesSeen = 0;
        var deadline = DateTime.UtcNow.AddSeconds(seconds);
        try
        {
            while (DateTime.UtcNow < deadline)
            {
                IntPtr headerPtr = IntPtr.Zero, dataPtr = IntPtr.Zero;
                var rc = pcap_next_ex(handle, ref headerPtr, ref dataPtr);
                if (rc == 0) continue;      // read timeout: the wire was quiet
                if (rc < 0) break;          // error or end of capture
                if (headerPtr == IntPtr.Zero || dataPtr == IntPtr.Zero) continue;

                var header = Marshal.PtrToStructure<PcapPktHdr>(headerPtr);
                var caplen = (int)header.CapLen;
                if (caplen < 14) continue;

                var frame = new byte[caplen];
                Marshal.Copy(dataPtr, frame, 0, caplen);

                var etherType = (ushort)((frame[12] << 8) | frame[13]);
                if (etherType != Tos64EtherType) continue;

                framesSeen++;
                var payload = frame[14..];
                payloads.Add(payload);
                if (sighted is not null && sighted(payload)) break;
            }
        }
        finally
        {
            pcap_close(handle);
        }
        return payloads;
    }

    private static string Str(byte[] errbuf)
    {
        var end = Array.IndexOf(errbuf, (byte)0);
        return System.Text.Encoding.ASCII.GetString(errbuf, 0, end < 0 ? errbuf.Length : end);
    }
}
