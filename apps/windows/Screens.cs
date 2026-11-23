using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using Microsoft.Win32;

namespace RunIt;

public sealed class Browse : Grid
{
    private readonly StackPanel rows = new();
    private readonly TextBlock status = Ui.Soft("");

    public Browse()
    {
        var s = Store.Current;
        var hint = Ui.Text("Search every model on Hugging Bay", Theme.Sans, 17, Theme.Soft).OneLine();
        hint.IsHitTestVisible = false;
        var box = new TextBox { Style = (Style)Application.Current.Resources["Field"], FontFamily = Theme.Sans, FontSize = 17, Text = s.Query };
        System.Windows.Automation.AutomationProperties.SetName(box, "Search every model on Hugging Bay");
        hint.Visibility = box.Text == "" ? Visibility.Visible : Visibility.Collapsed;
        box.TextChanged += (_, _) => { hint.Visibility = box.Text == "" ? Visibility.Visible : Visibility.Collapsed; if (box.Text != s.Query) s.Search(box.Text); };
        var field = new Border { Height = 54, CornerRadius = new(12), Background = Theme.Card, Padding = new(18, 0, 18, 0), Child = Ui.Columns([(new System.Windows.Shapes.Rectangle { Width = 10, Height = 10, Fill = Theme.Orange }.Mid(), -1), (new Grid { Children = { hint.Pad(3), box } }.Mid(), 0)], 14) };
        var top = Ui.Stack(field, status.Pad(0, 12, 0, 8));
        DockPanel.SetDock(top, Dock.Top);
        var scroll = Ui.Scroll(rows.Pad(0, 0, 0, 30));
        scroll.ScrollChanged += (_, e) => { if (e.VerticalOffset + e.ViewportHeight > e.ExtentHeight - 400 && e.ExtentHeight > e.ViewportHeight && s.Next is not null && !s.Searching) s.Search(s.Query, true); };
        Children.Add(new DockPanel { MaxWidth = 1120, Margin = new(36, 30, 36, 0), Children = { top, scroll } });
        List();
        this.Listen(t => { if (t is "search") { if (box.Text != s.Query) box.Text = s.Query; List(); } });
        Loaded += (_, _) => { box.Focus(); if (s.Hits.Count == 0) s.Search(s.Query); };
    }

    private void List()
    {
        var s = Store.Current;
        status.Text = s.Searching && s.Hits.Count == 0 ? "Searching" : s.Hits.Count == 0 ? s.Query == "" ? " " : "Nothing found. Try the publisher's name or fewer words." : $"{s.Total:N0} models";
        rows.Children.Clear();
        if (s.SearchError is { } e) { rows.Children.Add(Ui.Body(e, 14, Theme.Alarm).Pad(0, 12, 0, 12)); rows.Children.Add(Ui.Block("Try again", () => s.Search(s.Query))); return; }
        foreach (var h in s.Hits)
        {
            var sub = string.Join(", ", new[] { h.Repo.Split('/')[0], h.Kind, h.Label ?? h.Formats.FirstOrDefault(), h.Size is { } z ? Show.Size(z) : null, $"{Show.Count(h.Downloads)} downloads" }.Where(x => !string.IsNullOrEmpty(x)));
            rows.Children.Add(Ui.Line());
            rows.Children.Add(Rows.Model(h.Name, sub, h.Rating?.Score, h.Prediction, () => s.Open(new(h.Id, h.Name))));
        }
    }
}

public sealed class Library : Grid
{
    private readonly Dictionary<string, ItemProgress> live = [];
    private string shape = "";

    public Library()
    {
        Build();
        this.Listen(t => { if (t is "items") Build(); });
    }

    private void Build()
    {
        var s = Store.Current;
        // Rebuild only when something other than progress changed.
        var now = string.Join('|', s.Items.Select(i => $"{i.Key}{i.Status}{i.Measured?.TokensPerS}{i.Prediction?.TokensPerS}{s.Measuring.Contains(i.Key)}"));
        if (now == shape && shape != "") { foreach (var i in s.Items) if (live.TryGetValue(i.Key, out var p)) p.Show(i); return; }
        shape = now;
        live.Clear();
        Children.Clear();
        if (s.Items.Count == 0)
        {
            var about = Ui.Soft("Models you download live here, with their predicted speed beside the real one.", 14).Max(340).Centre().Pad(0, 10, 0, 20);
            about.TextAlignment = TextAlignment.Center;
            Children.Add(Ui.Stack(new Creature("empty", 120, Theme.Off).Centre(), Ui.Bold("Nothing here yet", 22).Centre().Pad(0, 18), about, Ui.Block("See what this PC runs", () => s.Go(Place.ForYou)).Centre()).Centre().Mid());
            return;
        }
        var list = Ui.Stack(Ui.Bold("Library", 27).Pad(0, 0, 0, 18));
        foreach (var i in s.Items) list.Children.Add(Row(i).Pad(0, 0, 0, 12));
        Children.Add(Ui.Page(list, 1080));
    }

    private Border Row(Item i)
    {
        var s = Store.Current;
        var busy = s.Measuring.Contains(i.Key);
        var measure = Ui.Plain(busy ? "Measuring" : i.Measured is null ? "Measure" : "Measure again", () => s.Measure(i.Key));
        measure.IsEnabled = !busy;
        var buttons = !i.Ready ? Ui.Row() : Ui.Row(
            i.Runnable ? measure : null,
            Ui.Plain("Show in Explorer", () => s.Reveal(i.Key)).Pad(8),
            Ui.Plain("Delete", () =>
            {
                if (MessageBox.Show($"Delete {i.Name}? This frees {Show.Size(i.Total)}. You can download it again.", "RunIt", MessageBoxButton.OKCancel, MessageBoxImage.None, MessageBoxResult.Cancel) == MessageBoxResult.OK) s.Act("remove", i.Key);
            }).Pad(8),
            i.Runnable ? Ui.Block("Try it", () => s.OpenChat(i.Key), s.Tint(i.Name)).Pad(8) : null);
        var body = Ui.Stack(Ui.Ends(Ui.Stack(Ui.Bold(i.Name, 19).OneLine(), Ui.Soft($"{i.Repo}, {i.Label}, {Show.Size(i.Total)}").OneLine().Pad(0, 4)).Pad(0, 0, 12), buttons.Top()));
        if (!i.Ready) { var p = new ItemProgress(i).Pad(0, 14); live[i.Key] = p; body.Children.Add(p); }
        else body.Children.Add(Versus(i, busy).Pad(0, 14));
        return Ui.Box(Ui.Columns([(Tile.Of(i.Name, 92).Top(), -1), (body, 0)], 20), pad: 18);
    }

    /// Predicted against real.
    private static StackPanel Versus(Item i, bool busy)
    {
        var top = Math.Max(1, Math.Max(i.Prediction?.TokensPerS ?? 0, i.Measured?.TokensPerS ?? 0));
        StackPanel Bar(string k, double v, Brush c) => Ui.Row(Ui.Soft(k, 13).Mid().Wide(86), new Blocks(v / top, 30, color: c), Ui.Pixel(Show.Speed(v), 25).Mid().Pad(14)).Pad(0, 0, 0, 5);
        var note = i.Measured?.Conditions is { } c ? c.Hot ? "Measured while this PC was hot and holding back. Let it cool and measure again." : c.OnBattery ? "Measured on battery." : null : null;
        return Ui.Stack(
            i.Prediction is { } p ? Bar("Predicted", p.TokensPerS, Theme.Soft) : null,
            i.Measured is { } m ? Bar("Measured", m.TokensPerS, Store.Current.Tint(i.Name)) : null,
            busy ? Ui.Soft("Timing it on this PC") : note is null ? null : Ui.Soft(note));
    }
}

public sealed class ThisPc : Grid
{
    public ThisPc()
    {
        var s = Store.Current;
        if (s.Report is not { } r) return;
        var m = r.Machine;
        var room = m.RamBytes - Math.Max(m.RamBytes / 4, 4 * 1073741824.0);
        var capacity = m.Gpu is { } g ? g.Integrated ? Math.Min(g.TotalMib * 1048576, room) : g.FreeMib * 1048576 : room;
        static FrameworkElement Stat(string v, string k, Brush c) => new Border { Height = 190, CornerRadius = new(14), Padding = new(18), Background = c, Child = Ui.Stack(Ui.Pixel(v, 62, Theme.Ink), Ui.Bold(k, 13, Theme.Ink)).Foot() };
        var list = Ui.Stack(
            Ui.Bold("This PC", 27),
            Ui.Soft($"{m.Cpu}{(m.Gpu is { } x ? ", " + x.Name : "")}, {m.Os}", 14).Pad(0, 6, 0, 20),
            Ui.Columns([
                (new Border { Height = 190, CornerRadius = new(14), Background = Theme.Orange, Child = new Creature(m.Cpu, 110).Centre().Mid() }, 1),
                (Stat($"{(int)(r.BytesPerS / 1e9)}", "GB of model read a second", Theme.Tints[1]), 1),
                (Stat(System.FormattableString.Invariant($"{capacity / 1073741824:0.0}"), "GB free for a model", Theme.Tints[2]), 1),
                (Stat($"{(int)(r.Calibration.Sustained * 100)}%", "of speed kept after 20 s", Theme.Tints[4]), 1)], 14),
            Ui.Soft("Writing a token means reading the model's weights once, so the first figure sets every speed in RunIt.", 13.5).Pad(0, 14),
            Ui.Bold("What was timed", 16).Pad(0, 32, 0, 10));
        var most = r.Runs.Count == 0 ? 1 : r.Runs.Max(y => y.Decode);
        foreach (var y in r.Runs)
        {
            list.Children.Add(Ui.Line());
            list.Children.Add(new Border
            {
                Height = 46,
                Child = Ui.Columns([(Ui.Bold($"{Show.Size(y.ModelBytes)} test model", 13.5).Mid().Wide(160), -1), (Ui.Soft(y.Place == "gpu" ? "graphics" : "processor", 13.5).Mid().Wide(80), -1),
                    (new Blocks(y.Decode / most, color: y.Place == "gpu" ? Theme.Orange : Theme.Tints[1]), 0), (Ui.Soft($"reads {Show.Speed(y.Prompt)}", 13).Mid(), -1),
                    (Ui.Row(Ui.Pixel(Show.Speed(y.Decode), 25), Ui.Soft("tok/s", 11).Foot().Pad(5, 0, 0, 5)).Right().Mid().Wide(96), -1)], 14),
            });
        }
        list.Children.Add(Ui.Row(Ui.Block("Test again", s.Test), Ui.Plain("Choose models folder", () =>
        {
            var d = new OpenFolderDialog { Title = "Choose where models go" };
            if (d.ShowDialog() == true) s.MoveModels(d.FolderName);
        }).Mid().Pad(10), Ui.Text(s.ModelsDir, Theme.Mono, 12, Theme.Soft).OneLine().Mid().Pad(12)).Pad(0, 22));
        list.Children.Add(Ui.Text($"Tested {DateTimeOffset.FromUnixTimeSeconds((long)r.TakenAt).LocalDateTime:g}, in {(int)r.Seconds} s. Models and files: Hugging Bay. Ratings: LMArena leaderboard dataset, CC BY 4.0, {s.RatingsDate}. Engine: llama.cpp. RunIt {s.Version} sends nothing about this computer anywhere.", Theme.Sans, 12.5, Theme.Soft, line: 18).Max(620).Pad(0, 32));
        Children.Add(Ui.Page(list, 1080));
    }
}

public sealed class Chat : DockPanel
{
    private readonly StackPanel turns = new() { MaxWidth = 700, Margin = new(36, 26, 36, 26) };
    private readonly ScrollViewer scroll;
    private readonly TextBlock speed, versus = Ui.Bold("", 12);
    private readonly TextBox box;
    private readonly Border send = new();
    private readonly Creature creature;
    private readonly string name;
    private TextBlock? reply;

    public Chat()
    {
        var s = Store.Current;
        var item = s.Items.FirstOrDefault(i => i.Key == s.ChatKey);
        name = item?.Name ?? "";
        var tint = s.Tint(name);
        speed = Ui.Pixel("", 48, tint);
        creature = new Creature(name, 40);
        var close = Ui.Plain("Close", s.CloseChat);
        close.IsCancel = true;
        var head = Ui.Columns([
            (new Border { Width = 58, Height = 58, CornerRadius = new(12), Background = tint, Child = creature.Centre().Mid() }, -1),
            (Ui.Stack(Ui.Bold(name, 19), Ui.Soft("Running on this PC. Nothing leaves it.").Pad(0, 3)).Mid(), 0),
            (Ui.Row(speed.Mid(), Ui.Stack(versus, Ui.Soft(item?.Prediction is { } p ? $"{Show.Speed(p.TokensPerS)} predicted" : "", 12)).Mid().Pad(8, 0, 18), close.Mid()), -1)], 16).Pad(36, 26, 36, 16);
        var top = Ui.Stack(head, Ui.Line());
        SetDock(top, Dock.Top);
        box = new TextBox { Style = (Style)Application.Current.Resources["Field"], FontFamily = Theme.Sans, FontSize = 15, TextWrapping = TextWrapping.Wrap, MaxLines = 6, VerticalAlignment = VerticalAlignment.Center, Margin = new(0, 4, 10, 4) };
        System.Windows.Automation.AutomationProperties.SetName(box, "Message");
        box.PreviewKeyDown += (_, e) => { if (e.Key == Key.Enter && Keyboard.Modifiers != ModifierKeys.Shift) { e.Handled = true; Send(); } };
        var bar = new Border { Background = Theme.Card, CornerRadius = new(12), Padding = new(18, 9, 10, 9), MinHeight = 54, MaxWidth = 736, Margin = new(36, 0, 36, 26), Child = Ui.Ends(box, send) };
        SetDock(bar, Dock.Bottom);
        scroll = Ui.Scroll(turns);
        Children.Add(top); Children.Add(bar); Children.Add(scroll);
        Build();
        this.Listen(t => { if (t is "chat") Build(); else if (t is "reply") Reply(); });
        Loaded += (_, _) => box.Focus();
    }

    private void Send()
    {
        var t = box.Text.Trim();
        if (t == "" || Store.Current.ChatBusy || Store.Current.ChatLoading) return;
        box.Clear();
        Store.Current.Send(t);
    }

    private void Build()
    {
        var s = Store.Current;
        turns.Children.Clear();
        reply = null;
        creature.Pace = s.ChatBusy || s.ChatLoading ? 0.14 : 0.46;
        if (s.ChatLoading) turns.Children.Add(new Typed(Ui.Text("Loading it into memory", Theme.Mono, 15, Theme.Soft), 30));
        else if (s.Turns.Count == 0 && s.ChatError is null) turns.Children.Add(Ui.Bold("Ask it anything.", 22, Theme.Soft));
        foreach (var t in s.Turns)
        {
            if (t.Mine) { turns.Children.Add(Ui.Box(Ui.Bold(t.Text, 15), radius: 10, pad: 12).Right().Pad(60, 0, 0, 20)); continue; }
            if (t.Thought != "")
            {
                var thought = Ui.Soft(t.Thought, 13).Pad(0, 4, 0, 10);
                thought.Visibility = Visibility.Collapsed;
                turns.Children.Add(Ui.Plain("Thinking", () => thought.Visibility = thought.Visibility == Visibility.Visible ? Visibility.Collapsed : Visibility.Visible).Pad(0, 0, 0, 8));
                turns.Children.Add(thought);
            }
            reply = Ui.Text(t.Text, Theme.Sans, 15.5, line: 25).Pad(0, 0, 0, 22);
            turns.Children.Add(reply);
        }
        if (s.ChatNote is { } n && !s.ChatBusy) turns.Children.Add(Ui.Soft(n));
        if (s.ChatError is { } e) turns.Children.Add(Ui.Body(e, 14, Theme.Alarm));
        send.Child = s.ChatBusy ? Ui.Plain("Stop", s.StopReply).Mid() : Ui.Block("Send", Send, s.Tint(name)).Mid();
        Reply();
    }

    private void Reply()
    {
        var s = Store.Current;
        if (s.LiveSpeed > 0) { speed.Text = Show.Speed(s.LiveSpeed); versus.Text = "tokens/s now"; }
        if (reply is null || s.Turns.Count == 0) return;
        var t = s.Turns[^1];
        reply.Inlines.Clear();
        reply.Inlines.Add(new System.Windows.Documents.Run(t.Text == "" && s.ChatBusy && t.Thought != "" ? "Thinking" : t.Text));
        if (s.ChatBusy) reply.Inlines.Add(new System.Windows.Documents.Run("  ") { Background = s.Tint(name) });
        scroll.ScrollToBottom();
    }
}
