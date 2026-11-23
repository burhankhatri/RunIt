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
    private readonly Border notice = new() { CornerRadius = new(8), Padding = new(16, 11, 16, 11), MaxWidth = 640, VerticalAlignment = VerticalAlignment.Bottom, HorizontalAlignment = HorizontalAlignment.Center, Margin = new(0, 0, 0, 22), Visibility = Visibility.Collapsed };
    private readonly DispatcherTimer hide = new() { Interval = TimeSpan.FromSeconds(8) };
    private string showing = "";

    public Shell()
    {
        (Title, Width, Height, MinWidth, MinHeight, Background) = ("RunIt", 1220, 800, 1060, 700, Theme.Paper);
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Icon = System.Windows.Media.Imaging.BitmapFrame.Create(new Uri("pack://application:,,,/RunIt.ico"));
        TextOptions.SetTextFormattingMode(this, TextFormattingMode.Ideal);
        UseLayoutRounding = true;
        notice.Background = Theme.Ink;
        Content = new Grid { Children = { root, notice } };
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
        UIElement? next = want switch
        {
            "fatal" => Ui.Stack(Ui.Head("Something is missing.", 44), Ui.Soft(s.Fatal!, 15).Pad(0, 14)).Max(560).Mid(),
            "opening" => new Opening(),
            "running" => new Running(),
            "main" => new MainScreen(),
            _ => null,
        };
        if (next is null) return;
        if (want == "fatal") ((FrameworkElement)next).HorizontalAlignment = HorizontalAlignment.Center;
        root.Children.Add(next);
        if (!Theme.Still) next.BeginAnimation(OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromSeconds(0.35)));
    }

    private void Notice()
    {
        hide.Stop();
        if (Store.Current.Notice is not { } n) { notice.Visibility = Visibility.Collapsed; return; }
        var dismiss = Ui.Bare(Ui.Body("Dismiss", 13, Theme.Dark ? Theme.OnMarker : Theme.Marker, true).Pad(6, 0, 6), () => Store.Current.Notify(null));
        notice.Child = Ui.Row(Ui.Body(n, 13, Theme.Paper).Max(500), dismiss.Pad(12));
        notice.Visibility = Visibility.Visible;
        notice.Rise();
        hide.Start();
    }
}

public sealed class MainScreen : Grid
{
    private readonly StackPanel nav = new();
    private readonly Border body = new();

    public MainScreen()
    {
        ColumnDefinitions.Add(new() { Width = new(172) });
        ColumnDefinitions.Add(new() { Width = new(1) });
        ColumnDefinitions.Add(new());
        var m = Store.Current.Report?.Machine;
        var foot = Ui.Stack(Ui.Body(m?.Cpu ?? "", 12), Ui.Soft(m is null ? "" : $"{Math.Round(m.RamBytes / 1073741824)} GB memory", 12)).Pad(10);
        DockPanel.SetDock(foot, Dock.Bottom);
        var side = new DockPanel { Margin = new(14, 24, 14, 22), Children = { foot, Ui.Stack(Ui.Text("RunIt", Theme.Display, 26, italic: true).Pad(10, 0, 0, 26), nav) } };
        var line = Ui.Tall();
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
            var label = Ui.Body(title, 14, on ? Theme.OnMarker : Theme.Ink, on).Mid();
            var count = Ui.Fig(place == Place.Library && busy > 0 ? $"{busy}" : "", 11, on ? Theme.OnMarker : Theme.Faint).Mid();
            var row = new Border { Height = 32, Padding = new(8, 0, 8, 0), CornerRadius = new(4), Background = on ? Theme.Marker : Brushes.Transparent, Child = Ui.Ends(label, count) };
            if (!on) { row.MouseEnter += (_, _) => row.Background = Theme.Wash; row.MouseLeave += (_, _) => row.Background = Brushes.Transparent; }
            nav.Children.Add(Ui.Bare(row, () => s.Go(place), title).Pad(0, 0, 0, 2));
        }
    }

    private void Body()
    {
        var s = Store.Current;
        FrameworkElement next = s.ChatKey is not null ? new Chat() : s.Place switch { Place.Browse => new Browse(), Place.Library => new Library(), Place.Machine => new ThisPc(), _ => new ForYou() };
        body.Child = next;
        if (!Theme.Still) next.BeginAnimation(OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromSeconds(0.22)));
    }
}

public sealed class Opening : Grid
{
    public Opening()
    {
        var s = Store.Current;
        var after = Ui.Stack(
            Ui.Text("RunIt builds test models on your disk and times the real engine on them, so the speeds it gives you belong to this computer. It takes a minute or two. Nothing is uploaded, and the test models are deleted.", Theme.Sans, 16, Theme.Faint, line: 25).Max(560),
            s.TestError is { } e ? Ui.Body(e, 14, Theme.Alarm).Max(560).Pad(0, 18) : null,
            Ui.Row(Ui.Solid("Test this PC", s.Test), s.Report is not null ? Ui.Outline("Keep the last result", s.Enter).Mid().Pad(14) : null).Pad(0, 26)).Pad(0, 26);
        after.Opacity = Theme.Still ? 1 : 0;
        var title = new Typed(Ui.Head("What can this PC run?", 68), 22, 0.35, true, () => { if (!Theme.Still) after.Rise(); });
        Children.Add(Ui.Stack(title, after).Pad(60, 0, 60, 120).Mid());
        MaxWidth = 940;
        if (after.Children.Count > 0 && ((StackPanel)after.Children[^1]).Children[0] is Button b) b.IsDefault = true;
    }
}

/// The lab report, writing itself.
public sealed class Running : Grid
{
    private readonly StackPanel lines = new() { VerticalAlignment = VerticalAlignment.Center };
    private readonly Border foot = new() { Height = 96, Margin = new(0, 0, 0, 30) };
    private readonly TextBlock stage = Ui.Fig("", 12, Theme.Faint), percent = Ui.Fig("", 12, Theme.Faint);
    private readonly (Grid View, Action<double> Set) bar = Ui.Bar(2, Theme.Ink);
    private int written;
    private string footing = "";

    public Running()
    {
        RowDefinitions.Add(new());
        RowDefinitions.Add(new() { Height = GridLength.Auto });
        (MaxWidth, Margin) = (820, new(60, 20, 60, 0));
        var scroll = Ui.Scroll(lines);
        scroll.ScrollChanged += (_, e) => { if (e.ExtentHeightChange > 0) scroll.ScrollToBottom(); };
        Children.Add(scroll);
        SetRow(foot, 1);
        Children.Add(foot);
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
            foreach (var old in lines.Children.OfType<Typed>()) old.Opacity = 0.55;
            lines.Children.Add(new Typed(Ui.Head(all[written], 29), 62).Pad(0, 0, 0, 13));
        }
    }

    private void Foot()
    {
        var s = Store.Current;
        var over = !s.Testing && s.TestError is null && s.Report is not null;
        var want = s.TestError is not null ? "error" : over ? s.Shortlist is not null ? "go" : s.ShortlistError is not null ? "offline" : "wait" : "run";
        if (want == "run")
        {
            stage.Text = s.Stage;
            percent.Text = $"{(int)(s.Progress * 100)}%";
            bar.Set(s.Progress);
        }
        if (want == "wait") { footing = want; foot.Child = new Border { Child = Ui.Body(s.CatalogProgress is { } p and < 1 ? $"Reading the catalog, {(int)(p * 100)}%" : "Matching models", 14, Theme.Faint) }; }
        if (want == footing) return;
        footing = want;
        if (want is "go" or "offline" or "wait") foreach (var l in lines.Children.OfType<Typed>()) l.Opacity = 1;
        switch (want)
        {
            case "error": foot.Child = Ui.Stack(Ui.Body(s.TestError!, 14, Theme.Alarm), Ui.Outline("Try again", s.Test).Pad(0, 14)); break;
            case "go": var b = Ui.Solid("See what it runs", s.Enter); b.IsDefault = true; foot.Child = new Border { Child = b, VerticalAlignment = VerticalAlignment.Top }.Rise(); break;
            case "offline": foot.Child = Ui.Stack(Ui.Body(s.ShortlistError!, 13, Theme.Alarm), Ui.Row(Ui.Solid("Continue", s.Enter), Ui.Outline("Try again", async () => await s.Recommend()).Mid().Pad(12)).Pad(0, 12)); break;
            case "run":
                (stage.Parent as Panel)?.Children.Clear();
                var right = Ui.Row(percent.Mid(), Ui.Outline("Stop", s.StopTest).Pad(14));
                foot.Child = Ui.Stack(bar.View, Ui.Ends(stage.Mid(), right).Pad(0, 12));
                break;
        }
    }
}
