using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace RunIt;

public sealed class CoreException(string message) : Exception(message);

/// The Rust engine. Calls may block on the network, so they run off the UI thread.
public sealed class Core : IDisposable
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void Callback(IntPtr context, IntPtr json);

    [DllImport("runit", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr runit_open(IntPtr engineDir, Callback callback, IntPtr context);
    [DllImport("runit", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr runit_call(IntPtr app, byte[] method, byte[] json);
    [DllImport("runit", CallingConvention = CallingConvention.Cdecl)]
    private static extern void runit_free(IntPtr text);
    [DllImport("runit", CallingConvention = CallingConvention.Cdecl)]
    private static extern void runit_close(IntPtr app);

    public static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower };

    private IntPtr app;
    // Held so the collector does not free what native code still calls.
    private readonly Callback callback;
    private readonly ReaderWriterLockSlim gate = new();

    public Core(Action<Event> onEvent)
    {
        callback = (_, p) =>
        {
            try
            {
                var e = JsonSerializer.Deserialize<Event>(Marshal.PtrToStringUTF8(p)!, Json);
                if (e is not null) onEvent(e);
            }
            catch (JsonException) { }
        };
        app = runit_open(IntPtr.Zero, callback, IntPtr.Zero);
        if (app == IntPtr.Zero) throw new CoreException("RunIt could not start. Check that this account can write to its AppData folder.");
    }

    private static byte[] Z(string s) => Encoding.UTF8.GetBytes(s + "\0");

    private JsonElement Ask(string method, object? p)
    {
        gate.EnterReadLock();
        try
        {
            if (app == IntPtr.Zero) throw new CoreException("RunIt is closing");
            var r = runit_call(app, Z(method), Z(JsonSerializer.Serialize(p ?? new { }, Json)));
            if (r == IntPtr.Zero) throw new CoreException("RunIt did not answer");
            string s;
            try { s = Marshal.PtrToStringUTF8(r)!; } finally { runit_free(r); }
            using var d = JsonDocument.Parse(s);
            if (d.RootElement.TryGetProperty("error", out var e)) throw new CoreException(e.GetString() ?? "unknown problem");
            return d.RootElement.GetProperty("ok").Clone();
        }
        finally { gate.ExitReadLock(); }
    }

    /// Two frames of ten rows. Pure arithmetic, so it answers at once.
    public int[][] Creature(string name) { try { return Ask("creature", new { name }).Deserialize<int[][]>(Json) ?? []; } catch (CoreException) { return []; } }

    public Task<T> Call<T>(string method, object? p = null) => Task.Run(() => Ask(method, p).Deserialize<T>(Json) ?? throw new CoreException("RunIt gave an empty answer"));
    public Task Run(string method, object? p = null) => Task.Run(() => { Ask(method, p); });

    public void Dispose()
    {
        // A call stuck on the network must not keep the app open.
        if (!gate.TryEnterWriteLock(1500)) return;
        try { if (app != IntPtr.Zero) runit_close(app); app = IntPtr.Zero; } finally { gate.ExitWriteLock(); }
    }
}
