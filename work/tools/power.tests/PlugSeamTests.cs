using System.Net;
using System.Net.Sockets;
using System.Text;

/// The HTTP seam, tested against a fake plug rather than declared thin.
///
/// The standing rule from 2026-08-03: **every declared-thin I/O seam gets
/// scripted platform-semantics tests** — the `Ok(0)` and timeout-class cases,
/// not just the happy path (`LE-66`). `tos64-power` has exactly one impure
/// component and this is it, so "thin" has to be demonstrated.
///
/// The fake is a `TcpListener` speaking the four lines of HTTP/1.1 this needs,
/// not an `HttpListener`: `HttpListener` prefixes need a URL ACL or an elevated
/// process on Windows, and a test that only runs elevated is a test nobody
/// runs — which is `LE-92`'s lesson (a gate that needed QEMU was a gate nobody
/// ran, and that is how the hole stayed open for months).
public sealed class PlugSeamTests
{
    /// A plug that answers correctly is the only case where a state is named.
    [Fact]
    public async Task A_plug_that_answers_names_its_state()
    {
        using var plug = FakePlug.Answering("{\"POWER\":\"ON\"}");
        var reply = await Client().Fetch(PlugDialect.Request(PlugKind.Tasmota, plug.Base, PlugAction.Read));
        Assert.Equal(PlugState.On, PlugDialect.ReadState(PlugKind.Tasmota, reply.Body ?? ""));
        Assert.True(reply.Reached);
    }

    /// `Ok(0)` — the exact shape this project has been bitten by on its other
    /// stream seams. A 200 with an empty body is a plug that answered and said
    /// nothing, and the only honest reading is Unknown.
    [Fact]
    public async Task An_empty_two_hundred_is_Unknown_not_success()
    {
        using var plug = FakePlug.Answering("");
        var reply = await Client().Fetch(PlugDialect.Request(PlugKind.Tasmota, plug.Base, PlugAction.Read));
        Assert.True(reply.Reached);
        Assert.Equal(PlugState.Unknown, PlugDialect.ReadState(PlugKind.Tasmota, reply.Body ?? ""));
    }

    [Fact]
    public async Task A_five_hundred_is_reached_but_unreadable()
    {
        using var plug = FakePlug.Failing(500, "internal error");
        var reply = await Client().Fetch(PlugDialect.Request(PlugKind.Tasmota, plug.Base, PlugAction.Read));
        Assert.False(reply.Ok);
        Assert.Equal(PlugState.Unknown, PlugDialect.ReadState(PlugKind.Tasmota, reply.Body ?? ""));
    }

    /// The timeout class, and the reason it is bounded rather than left to
    /// `HttpClient`'s 100-second default: a `cycle` holds the board OFF while
    /// it waits, so an unbounded wait on the ON leg is the fail-safe's own
    /// worst case. A plug that stops answering must cost seconds, not minutes.
    [Fact]
    public async Task A_plug_that_never_answers_times_out_bounded_and_does_not_throw()
    {
        using var plug = FakePlug.Silent();
        var started = DateTimeOffset.UtcNow;
        var reply = await Client(TimeSpan.FromSeconds(2))
            .Fetch(PlugDialect.Request(PlugKind.Tasmota, plug.Base, PlugAction.Read));
        var took = DateTimeOffset.UtcNow - started;

        Assert.False(reply.Reached);
        Assert.Null(reply.Body);
        Assert.True(took < TimeSpan.FromSeconds(15), $"the seam waited {took.TotalSeconds:F1}s");
        Assert.NotNull(reply.Failure);
    }

    /// Nothing listening at all: the plug is unplugged, or on the other subnet,
    /// or the operator typed the wrong address. `Reached=false` and a message,
    /// never an exception escaping into a `cycle` that has already switched
    /// power off.
    [Fact]
    public async Task Nothing_listening_is_a_reply_and_not_an_exception()
    {
        var deadPort = FakePlug.APortNobodyIsOn();
        var plugBase = PlugBase.Parse($"http://127.0.0.1:{deadPort}", null)!;
        var reply = await Client(TimeSpan.FromSeconds(2))
            .Fetch(PlugDialect.Request(PlugKind.Tasmota, plugBase, PlugAction.Read));

        Assert.False(reply.Reached);
        Assert.NotNull(reply.Failure);
    }

    /// A truncated answer — connection closed mid-body. It reads as a partial
    /// success to anything that checks only the status line, which is the one
    /// mistake this whole file exists to make impossible.
    [Fact]
    public async Task A_connection_dropped_mid_body_is_not_a_state()
    {
        using var plug = FakePlug.Truncating("{\"POWER\":\"O");
        var reply = await Client(TimeSpan.FromSeconds(2))
            .Fetch(PlugDialect.Request(PlugKind.Tasmota, plug.Base, PlugAction.Read));
        Assert.Equal(PlugState.Unknown, PlugDialect.ReadState(PlugKind.Tasmota, reply.Body ?? ""));
    }

    /// The POST leg is exercised too. ESPHome's actions are POSTs and a client
    /// that quietly sent a GET would get a 405 that reads like a dead plug.
    [Fact]
    public async Task The_post_leg_reaches_the_plug_as_a_post()
    {
        using var plug = FakePlug.Echoing();
        var plugBase = PlugBase.Parse(plug.BaseUrl, "board", PlugKind.Esphome)!;
        var reply = await Client().Fetch(PlugDialect.Request(PlugKind.Esphome, plugBase, PlugAction.On));
        Assert.True(reply.Reached);
        Assert.StartsWith("POST /switch/board/turn_on", reply.Body);
    }

    private static PlugClient Client(TimeSpan? timeout = null) =>
        new(timeout ?? TimeSpan.FromSeconds(5));

    /// Four lines of HTTP/1.1 over a loopback `TcpListener`. Deliberately not a
    /// web server: every behaviour under test here is a way of NOT answering
    /// properly, and a real server is built to avoid producing them.
    private sealed class FakePlug : IDisposable
    {
        private readonly TcpListener _listener;
        private readonly CancellationTokenSource _stop = new();

        private FakePlug(Func<string, (int Status, string? Body, bool Truncate, bool Silent)> respond)
        {
            _listener = new TcpListener(IPAddress.Loopback, 0);
            _listener.Start();
            _ = Task.Run(() => Serve(respond));
        }

        internal int Port => ((IPEndPoint)_listener.LocalEndpoint).Port;
        internal string BaseUrl => $"http://127.0.0.1:{Port}";
        internal PlugBase Base => PlugBase.Parse(BaseUrl, null)!;

        internal static FakePlug Answering(string body) => new(_ => (200, body, false, false));
        internal static FakePlug Failing(int status, string body) => new(_ => (status, body, false, false));
        internal static FakePlug Silent() => new(_ => (0, null, false, true));
        internal static FakePlug Truncating(string partial) => new(_ => (200, partial, true, false));
        internal static FakePlug Echoing() => new(request => (200, request, false, false));

        /// A port with nothing on it, obtained by binding one and letting it go.
        /// Racy in principle and fine in practice on a loopback bench; the test
        /// that uses it asserts "did not reach", which a stray listener would
        /// have to answer valid Tasmota JSON to break.
        internal static int APortNobodyIsOn()
        {
            var probe = new TcpListener(IPAddress.Loopback, 0);
            probe.Start();
            var port = ((IPEndPoint)probe.LocalEndpoint).Port;
            probe.Stop();
            return port;
        }

        private async Task Serve(Func<string, (int Status, string? Body, bool Truncate, bool Silent)> respond)
        {
            while (!_stop.IsCancellationRequested)
            {
                TcpClient client;
                try { client = await _listener.AcceptTcpClientAsync(_stop.Token); }
                catch (OperationCanceledException) { return; }
                catch (SocketException) { return; }
                catch (ObjectDisposedException) { return; }

                _ = Task.Run(async () =>
                {
                    using var owned = client;
                    var stream = owned.GetStream();
                    var buffer = new byte[4096];
                    int read;
                    try { read = await stream.ReadAsync(buffer, _stop.Token); }
                    catch (Exception) { return; }
                    var requestLine = Encoding.ASCII.GetString(buffer, 0, read).Split("\r\n")[0];

                    var (status, body, truncate, silent) = respond(requestLine);
                    if (silent)
                    {
                        // Hold the connection open and say nothing: the caller's
                        // own deadline is what has to end this.
                        try { await Task.Delay(Timeout.Infinite, _stop.Token); } catch { }
                        return;
                    }

                    var payload = Encoding.UTF8.GetBytes(body ?? "");
                    var head = Encoding.ASCII.GetBytes(
                        $"HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n" +
                        // A truncating plug PROMISES more than it sends, which
                        // is what makes the drop look like a partial success.
                        $"Content-Length: {(truncate ? payload.Length + 32 : payload.Length)}\r\n\r\n");
                    try
                    {
                        await stream.WriteAsync(head, _stop.Token);
                        await stream.WriteAsync(payload, _stop.Token);
                        await stream.FlushAsync(_stop.Token);
                    }
                    catch (Exception) { /* the client gave up first; that is the test */ }
                });
            }
        }

        public void Dispose()
        {
            _stop.Cancel();
            _listener.Stop();
            _stop.Dispose();
        }
    }
}
