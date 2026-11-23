using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;
using System.Windows.Threading;

namespace RunIt;

public sealed class Shell : Window
{
    private readonly Grid root = new();
    private readonly Border notice = new() { CornerRadius = new(10), Padding = new(16, 12, 16, 12), MaxWidth = 640, VerticalAlignment = VerticalAlignment.Bottom, HorizontalAlignment = HorizontalAlignment.Center, Margin = new(0, 0, 0, 22), Visibility = Visibility.Collapsed };
    private readonly DispatcherTimer hide = new() { Interval = TimeSpan.FromSeconds(8) };
    private string showing = "";

    public Shell()
    {
        (Title, Width, Height, MinWidth, MinHeight, Background) = ("RunIt", 1200, 780, 980, 660, Theme.Ground);
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Icon = System.Windows.Media.Imaging.BitmapFrame.Create(new Uri("pack://application:,,,/RunIt.ico"));
        TextOptions.SetTextFormattingMode(this, TextFormattingMode.Ideal);
        UseLayoutRounding = true;
        notice.Background = Theme.Text;
        Content = new Grid { Background = Theme.Ground, Children = { root, notice } };
        SourceInitialized += (_, _) => Theme.TitleBar(this);
        hide.Tick += (_, _) => Store.Current.Notify(null);
        Store.Current.Changed += On;
        foreach (var (key, place) in new[] { (Key.D1, Place.ForYou), (Key.D2, Place.Browse), (Key.D3, Place.Library), (Key.D4, Place.Machine) })
        {
            var c = new RoutedCommand();
            CommandBindings.Add(new(c, (_, _) => { if (Store.Current.Entered) Store.Current.Go(place); }));
            InputBindings.Add(new(c, new KeyGesture(key, ModifierKeys.Control)));
        }
    }

    private void On(string topic)
    {
        if (topic == "notice") { Notice(); return; }
        if (topic != "shell") return;
        var s = Store.Current;
        var want = s.Fatal is not null ? "fatal" : !s.Ready ? "" : s.Entered ? "main" : s.Testing || s.Lines.Count > 0 ? "running" : "opening";
        if (want == showing && want != "opening") return;
        showing = want;
        root.Children.Clear();
        FrameworkElement? next = want switch
        {
            "fatal" => Ui.Stack(Ui.Bold("Something is missing", 28), Ui.Soft(s.Fatal!, 15).Pad(0, 14)).Max(520).Mid().Centre(),
            "opening" => new Opening(),
            "running" => new Running(),
            "main" => new MainScreen(),
            _ => null,
        };
        if (next is null) return;
        root.Children.Add(next);
        if (!Theme.Still) next.BeginAnimation(OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromSeconds(0.3)));
    }

    private void Notice()
    {
        hide.Stop();
        if (Store.Current.Notice is not { } n) { notice.Visibility = Visibility.Collapsed; return; }
        notice.Child = Ui.Row(Ui.Body(n, 13, Theme.Ink).Max(500), Ui.Bare(Ui.Bold("Dismiss", 13, Theme.Ink).Pad(6, 0, 6), () => Store.Current.Notify(null)).Pad(12));
        notice.Visibility = Visibility.Visible;
        notice.Rise();
        hide.Start();
    }
}

public sealed class MainScreen : Grid
{
    private readonly StackPanel nav = new();
    private readonly Border body = new() { ClipToBounds = true };

    public MainScreen()
    {
        ColumnDefinitions.Add(new() { Width = new(198) });
        ColumnDefinitions.Add(new() { Width = new(1) });
        ColumnDefinitions.Add(new());
        var cpu = Store.Current.Report?.Machine.Cpu ?? "this PC";
        var ram = Math.Round((Store.Current.Report?.Machine.RamBytes ?? 0) / 1073741824);
        var foot = Ui.Columns([(new Creature(cpu, 30, Theme.Orange).Mid(), -1), (Ui.Stack(Ui.Bold(cpu, 12), Ui.Soft($"{ram} GB memory", 12).Pad(0, 2)).Pad(10), 0)]).Pad(8);
        DockPanel.SetDock(foot, Dock.Bottom);
        var mark = new Border { Width = 30, Height = 30, CornerRadius = new(7), Background = Theme.Orange, Child = new Creature("RunIt", 20).Centre().Mid() };
        var side = new DockPanel { Margin = new(12, 24, 12, 20), Children = { foot, Ui.Stack(Ui.Row(mark, Ui.Pixel("RunIt", 27).Mid().Pad(10)).Pad(8, 0, 0, 20), nav) } };
        var line = new Rectangle { Width = 1, Fill = Theme.Line, SnapsToDevicePixels = true };
        SetColumn(line, 1);
        SetColumn(body, 2);
        Children.Add(side); Children.Add(line); Children.Add(body);
        Nav(); Body();
        this.Listen(t => { if (t is "place") { Nav(); Body(); } else if (t is "items") Nav(); });
    }

    private void Nav()
    {
        var s = Store.Current;
        nav.Children.Clear();
        var busy = s.Items.Count(i => i.Status is "active" or "queued");
        foreach (var (title, place) in new[] { ("For you", Place.ForYou), ("Browse", Place.Browse), ("Library", Place.Library), ("This PC", Place.Machine) })
        {
            var on = s.Place == place && s.ChatKey is null;
            var label = Ui.Text(title, Theme.Sans, 14, on ? Theme.Text : Theme.Soft, on).Mid();
            var row = new Border { Height = 35, Padding = new(8, 0, 8, 0), CornerRadius = new(8), Background = on ? Theme.Raised : Brushes.Transparent, Child = Ui.Ends(label, Ui.Pixel(place == Place.Library && busy > 0 ? $"{busy}" : "", 16, Theme.Orange).Mid()) };
            nav.Children.Add(Ui.Bare(row, () => s.Go(place), title).Pad(0, 0, 0, 2));
        }
    }

    private void Body()
    {
        var s = Store.Current;
        FrameworkElement next = s.ChatKey is not null ? new Chat() : s.Opened is { } o ? new ModelPage(o) : s.Place switch { Place.Browse => new Browse(), Place.Library => new Library(), Place.Machine => new ThisPc(), _ => new ForYou() };
        body.Child = next;
        if (s.Opened is not null && s.ChatKey is null) next.Rise(0, 40);
        else if (!Theme.Still) next.BeginAnimation(OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromSeconds(0.2)));
    }
}

public sealed class Opening : Grid
{
    public Opening()
    {
        var s = Store.Current;
        var start = Ui.Block("Test this PC", s.Test, tall: true);
        start.IsDefault = true;
        var after = Ui.Stack(
            Ui.Text("RunIt builds small test models on your disk and times the real engine on them, so the speeds belong to this computer. It takes a minute or two. Nothing is uploaded, and the test models are deleted.", Theme.Sans, 15, Theme.Soft, line: 24).Max(440),
            s.TestError is { } e ? Ui.Body(e, 14, Theme.Alarm).Max(440).Pad(0, 18) : null,
            Ui.Row(start, s.Report is not null ? Ui.Plain("Keep the last result", s.Enter).Mid().Pad(12) : null).Pad(0, 24)).Pad(0, 24);
        after.Opacity = Theme.Still ? 1 : 0;
        var title = new Typed(Ui.Text("What can\nthis PC run?", Theme.Sans, 52, bold: true, line: 60), 20, 0.3, true, () => { if (!Theme.Still) after.Rise(0, 10); });
        var art = new Border { Width = 320, Height = 320, CornerRadius = new(28), Background = Theme.Orange, Child = new Creature("RunIt", 220).Centre().Mid() };
        Children.Add(Ui.Row(art.Mid(), Ui.Stack(title, after).Wide(470).Mid().Pad(60)).Centre().Mid());
    }
}

/// The machine's creature goes up block by block while the findings type out.
public sealed class Running : Grid
{
    private readonly StackPanel lines = new();
    private readonly Border foot = new() { Margin = new(0, 30, 0, 0) }, art = new() { Width = 320, Height = 320 };
    private readonly TextBlock stage = Ui.Soft("", 13), percent = Ui.Pixel("0%", 44, Theme.Orange);
    private int written;
    private string footing = "";

    public Running()
    {
        stage.TextAlignment = TextAlignment.Right;
        art.Child = new Building(Store.Current.Report?.Machine.Cpu ?? "this PC", 320);
        var left = Ui.Stack(art, Ui.Ends(percent, stage.Mid()).Wide(320).Pad(0, 22));
        var right = Ui.Stack(lines, foot).Wide(470).Mid().Pad(60);
        Children.Add(Ui.Row(left.Mid(), right).Centre().Mid());
        Lines(); Foot();
        this.Listen(t =>
        {
            if (t is "line") Lines();
            if (t is "line" or "test" or "shell" or "catalog" or "shortlist") Foot();
        });
    }

    private void Lines()
    {
        var all = Store.Current.Lines;
        if (written > all.Count) { lines.Children.Clear(); written = 0; }
        for (; written < all.Count; written++)
        {
            foreach (var old in lines.Children.OfType<Grid>()) old.Opacity = 0.6;
            var dot = new Rectangle { Width = 9, Height = 9, Fill = Theme.Orange, SnapsToDevicePixels = true };
            lines.Children.Add(Ui.Columns([(dot.Top().Pad(0, 7), -1), (new Typed(Ui.Text(all[written], Theme.Mono, 15, line: 22), 70), 0)], 12).Pad(0, 0, 0, 13));
        }
    }

    private void Foot()
    {
        var s = Store.Current;
        var over = !s.Testing && s.TestError is null && s.Report is not null;
        var want = s.TestError is not null ? "error" : over ? s.Shortlist is not null ? "go" : s.ShortlistError is not null ? "offline" : "wait" : "run";
        percent.Text = $"{(int)(s.Progress * 100)}%";
        stage.Text = over ? "Done" : s.Stage;
        if (want == "wait") foot.Child = Ui.Bold(s.CatalogProgress is { } p and < 1 ? $"Reading the catalog, {(int)(p * 100)}%" : "Matching models", 15, Theme.Soft);
        if (want == footing) return;
        footing = want;
        if (over)
        {
            foreach (var l in lines.Children.OfType<Grid>()) l.Opacity = 1;
            if (art.Child is Building)
            {
                (art.Background, art.CornerRadius) = (Theme.Orange, new(28));
                art.Child = new Creature(s.Report!.Machine.Cpu, 220, pace: 0.3).Centre().Mid();
            }
        }
        switch (want)
        {
            case "error": foot.Child = Ui.Stack(Ui.Body(s.TestError!, 14, Theme.Alarm), Ui.Block("Try again", s.Test).Pad(0, 14)); break;
            case "go": var b = Ui.Block("See what it runs", s.Enter, tall: true); b.IsDefault = true; foot.Child = b.Rise(); break;
            case "offline": foot.Child = Ui.Stack(Ui.Body(s.ShortlistError!, 13, Theme.Alarm), Ui.Row(Ui.Block("Continue", s.Enter), Ui.Plain("Try again", async () => await s.Recommend()).Mid().Pad(12)).Pad(0, 12)); break;
            case "run": foot.Child = Ui.Plain("Stop", s.StopTest); break;
        }
    }
}
