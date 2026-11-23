using System.IO;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace RunIt;

/// With RUNIT_SHOTS set, RunIt walks through its own screens and saves pictures of its own window.
/// This is how the release build is checked in CI.
public static class Rehearsal
{
    public static readonly string? Folder = Environment.GetEnvironmentVariable("RUNIT_SHOTS");

    public static async void Begin()
    {
        if (Folder is null) return;
        Directory.CreateDirectory(Folder);
        try { await Walk(); } catch (Exception e) { Fail(e.ToString()); return; }
        Say("REHEARSAL OK");
        Application.Current.Shutdown(0);
    }

    public static void Fail(string why) { Say("FAILED " + why); Environment.Exit(1); }
    private static void Say(string s) => File.AppendAllText(Path.Combine(Folder!, "log.txt"), s + Environment.NewLine);

    private static async Task Walk()
    {
        var s = Store.Current;
        await Until("the app to open", 30, () => s.Ready || s.Fatal is not null);
        if (s.Fatal is { } f) throw new CoreException(f);
        if (s.Report is null)
        {
            await Task.Delay(700);
            Shot("open-typing");
            await Task.Delay(2500);
            Shot("open");
            s.Test();
            for (var n = 1; n <= 4; n++)
            {
                var want = n * 2;
                await Until("the test to move on", 900, () => s.Lines.Count >= want || !s.Testing);
                await Task.Delay(n == 2 ? 350 : 1200);
                Shot($"test-{n}");
            }
            await Until("the test to finish", 1500, () => !s.Testing);
            if (s.TestError is { } e) throw new CoreException(e);
            await Task.Delay(2000);
            Shot("test-done");
        }
        await Until("suggestions", 300, () => s.Shortlist is not null || s.ShortlistError is not null);
        if (s.ShortlistError is { } x) throw new CoreException(x);
        if (s.Shortlist!.Ranked.Count + s.Shortlist.Unrated.Count == 0) throw new CoreException("no suggestions");
        s.Enter();
        await Task.Delay(1000);
        Shot("for-you-arriving");
        await Task.Delay(6000);
        Shot("for-you");
        var first = s.Shortlist.Picks.Select(p => p.Choice).Concat(s.Shortlist.Ranked).Concat(s.Shortlist.Unrated).First();
        s.Open(new(first.Id, first.Name, first));
        await Task.Delay(7000);
        Shot("model");
        s.Go(Place.Machine);
        await Task.Delay(1500);
        Shot("this-pc");
        s.Go(Place.Browse);
        await Until("search", 60, () => s.Hits.Count > 0 || s.SearchError is not null);
        s.Search("gemma 3");
        await Task.Delay(5000);
        if (s.SearchError is { } y) throw new CoreException(y);
        Shot("browse");
        if (Environment.GetEnvironmentVariable("RUNIT_FETCH") is { } id) await Fetch(s, id);
        s.Go(Place.Library);
        await Task.Delay(1000);
        Shot("library");
        if (s.Items.FirstOrDefault(i => i.Runnable) is not { } item) return;
        s.Measure(item.Key);
        await Until("a measurement", 400, () => s.Measuring.Count == 0);
        if (s.Items.First(i => i.Key == item.Key).Measured is null) throw new CoreException("nothing was measured: " + s.Notice);
        await Task.Delay(1000);
        Shot("library-measured");
        s.OpenChat(item.Key);
        await Until("the model to load", 180, () => !s.ChatLoading);
        s.Send("In one sentence, why is the sky blue?");
        await Until("a reply", 300, () => !s.ChatBusy);
        if (s.ChatError is { } z) throw new CoreException(z);
        if (s.Turns[^1].Text == "" && s.Turns[^1].Thought == "") throw new CoreException("empty reply");
        await Task.Delay(800);
        Shot("chat");
        s.CloseChat();
        await Task.Delay(500);
    }

    private static async Task Fetch(Store s, string id)
    {
        var d = await s.Core!.Call<Detail>("model", new { id });
        s.Download(id, d.Variants[0].Variant.Files);
        await Until("the download to start", 60, () => s.Items.Count > 0);
        s.Go(Place.Library);
        await Task.Delay(1500);
        Shot("downloading");
        await Until("the download", 900, () => s.Items.All(i => i.Ready || i.Status == "failed"));
        if (s.Items.FirstOrDefault(i => i.Status == "failed") is { } bad) throw new CoreException(bad.Error ?? "failed");
    }

    private static async Task Until(string what, double limit, Func<bool> done)
    {
        var start = DateTime.UtcNow;
        while (!done())
        {
            if ((DateTime.UtcNow - start).TotalSeconds > limit) throw new TimeoutException("gave up waiting for " + what);
            await Task.Delay(100);
        }
    }

    private static void Shot(string name)
    {
        var v = (FrameworkElement)Application.Current.MainWindow.Content;
        var b = new RenderTargetBitmap((int)v.ActualWidth, (int)v.ActualHeight, 96, 96, PixelFormats.Pbgra32);
        var d = new DrawingVisual();
        using (var c = d.RenderOpen())
        {
            c.DrawRectangle(Theme.Ground, null, new(0, 0, v.ActualWidth, v.ActualHeight));
            c.DrawRectangle(new VisualBrush(v) { ViewboxUnits = BrushMappingMode.Absolute, Viewbox = new(0, 0, v.ActualWidth, v.ActualHeight) }, null, new(0, 0, v.ActualWidth, v.ActualHeight));
        }
        b.Render(d);
        var png = new PngBitmapEncoder { Frames = { BitmapFrame.Create(b) } };
        using var file = File.Create(Path.Combine(Folder!, name + ".png"));
        png.Save(file);
        Say(name);
    }
}
