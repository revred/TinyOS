using System.Net.Http;

/// The one impure component in `tos64-power`, kept to one file so "thin" is a
/// claim a reviewer can check rather than take.
///
/// The standing rule from 2026-08-03 — every declared-thin I/O seam gets
/// scripted platform-semantics tests, `Ok(0)` and the timeout class included
/// (`LE-66`) — is discharged in `PlugSeamTests` against a loopback fake plug,
/// including: an empty 200, a 500, a body truncated mid-flight, nothing
/// listening at all, and a plug that accepts the connection and then says
/// nothing forever.
internal sealed record PlugReply(bool Reached, bool Ok, string? Body, string? Failure);

internal sealed class PlugClient
{
    private readonly HttpClient _http;

    /// Bounded, and NOT by `HttpClient`'s hundred-second default. A `cycle`
    /// holds the board off while it waits for the on leg, so an unbounded wait
    /// here is the fail-safe's own worst case — the tool would sit for minutes
    /// with the bench dark, which is the state clause 1 exists to prevent.
    internal PlugClient(TimeSpan timeout)
    {
        _http = new HttpClient { Timeout = timeout };
    }

    /// One request, one reply, no exceptions.
    ///
    /// Every failure comes back as a `PlugReply` with `Reached=false` and a
    /// message, because the caller may be holding the board OFF when this
    /// returns and an exception thrown through that path is a bench left dark
    /// by a stack unwind.
    internal async Task<PlugReply> Fetch(PlugRequest request)
    {
        try
        {
            using var message = new HttpRequestMessage(request.Method, request.Url);
            using var response = await _http.SendAsync(message);
            var body = await response.Content.ReadAsStringAsync();
            return new PlugReply(true, response.IsSuccessStatusCode, body,
                response.IsSuccessStatusCode ? null : $"HTTP {(int)response.StatusCode}");
        }
        catch (TaskCanceledException)
        {
            // The deadline, spelled as its own case: "the plug did not answer
            // in time" and "the plug refused the connection" are different
            // findings on a bench and the operator gets to see which.
            return new PlugReply(false, false, null, $"no answer within {_http.Timeout.TotalSeconds:F0}s");
        }
        catch (HttpRequestException e)
        {
            return new PlugReply(false, false, null, e.Message);
        }
        catch (IOException e)
        {
            // A body cut off mid-flight. It reads as a partial success to
            // anything that checks only the status line, and a partial success
            // on this seam is `LE-87` again.
            return new PlugReply(false, false, null, e.Message);
        }
        catch (UriFormatException e)
        {
            return new PlugReply(false, false, null, e.Message);
        }
        catch (InvalidOperationException e)
        {
            return new PlugReply(false, false, null, e.Message);
        }
    }
}
