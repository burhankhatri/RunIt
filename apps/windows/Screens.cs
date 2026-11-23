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
    private readonly Border pane = new() { Width = 410 };
    private readonly ScrollViewer scroll;

    public Browse()
    {
        var s = Store.Current;
        ColumnDefinitions.Add(new());
        ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        var hint = Ui.Text("Search every model on Hugging Bay", Theme.Display, 30, Theme.Faint);
        hint.IsHitTestVisible = false;
        var box = new TextBox { Style = (Style)Application.Current.Resources["Field"], FontFamily = Theme.Display, FontSize = 30, Text = s.Query };
        System.Windows.Automation.AutomationProperties.SetName(box, "Search every model on Hugging Bay");
        hint.Visibility = box.Text == "" ? Visibility.Visible : Visibility.Collapsed;
        box.TextChanged += (_, _) => { hint.Visibility = box.Text == "" ? Visibility.Visible : Visibility.Collapsed; s.Search(box.Text); };
        var top = Ui.Stack(new Grid { Children = { hint.Pad(3), box } }, status.Pad(0, 8, 0, 14), Ui.Line());
        DockPanel.SetDock(top, Dock.Top);
        scroll = Ui.Scroll(rows.Pad(0, 0, 0, 30));
        scroll.ScrollChanged += (_, e) => { if (e.VerticalOffset + e.ViewportHeight > e.ExtentHeight - 400 && e.ExtentHeight > e.ViewportHeight && s.Next is not null && !s.Searching) s.Search(s.Query, true); };
        Children.Add(new DockPanel { Margin = new(40, 34, 40, 0), Children = { top, scroll } });
        SetColumn(pane, 1);
        Children.Add(pane);
        Rows_(); Pane();
        this.Listen(t => { if (t is "search") Rows_(); if (t is "browsing") { Rows_(); Pane(); } });
        Loaded += (_, _) => { box.Focus(); if (s.Hits.Count == 0) s.Search(s.Query); };
    }

    private void Rows_()
    {
        var s = Store.Current;
        status.Text = s.Searching && s.Hits.Count == 0 ? "Searching" : s.Hits.Count == 0 ? s.Query == "" ? "" : "Nothing found. Try the publisher's name or fewer words." : $"{s.Total:N0} models";
        rows.Children.Clear();
        if (s.SearchError is { } e) { rows.Children.Add(Ui.Body(e, 14, Theme.Alarm).Pad(0, 24, 0, 12)); rows.Children.Add(Ui.Outline("Try again", () => s.Search(s.Query))); return; }
        foreach (var h in s.Hits)
        {
            var sub = string.Join(", ", new[] { h.Repo.Split('/')[0], h.Kind, h.Label ?? h.Formats.FirstOrDefault(), h.Size is { } z ? Show.Size(z) : null, $"{Show.Count(h.Downloads)} downloads" }.Where(x => !string.IsNullOrEmpty(x)));
            rows.Children.Add(Rows.Model(h.Name, sub, h.Rating?.Score, h.Prediction, s.Browsing?.Id == h.Id, () => s.Select(h)));
            rows.Children.Add(Ui.Line());
        }
    }

    private void Pane()
    {
        var h = Store.Current.Browsing;
        pane.Visibility = h is null ? Visibility.Collapsed : Visibility.Visible;
        pane.Child = h is null ? null : new DetailPane(h.Id, null, false);
    }
}

public sealed class Library : Grid
{
    private readonly StackPanel list = new() { MaxWidth = 820, HorizontalAlignment = HorizontalAlignment.Stretch, Margin = new(40, 34, 40, 40) };
    private readonly Dictionary<string, ItemProgress> live = [];
    private string shape = "";

    public Library()
    {
        list.HorizontalAlignment = HorizontalAlignment.Left;
        list.Width = 820;
        Children.Add(Ui.Scroll(list));
        Build();
        this.Listen(t => { if (t is "items") Build(); });
    }

    private void Build()
    {
        var s = Store.Current;
        // Rebuild only when something other than progress changed.
        var now = string.Join('|', s.Items.Select(i => $"{i.Key}{i.Status}{i.Measured?.TokensPerS}{s.Measuring.Contains(i.Key)}"));
        if (now == shape && shape != "") { foreach (var i in s.Items) if (live.TryGetValue(i.Key, out var p)) p.Show(i); return; }
        shape = now;
        live.Clear();
        list.Children.Clear();
        list.Children.Add(Ui.Head("Library", 46).Pad(0, 0, 0, 24));
        if (s.Items.Count == 0)
        {
            list.Children.Add(Ui.Soft("Models you download are kept here, with their predicted speed beside the real one.", 15).Max(440));
            list.Children.Add(Ui.Solid("See what this PC runs", () => s.Go(Place.ForYou)).Pad(0, 20));
            return;
        }
        list.Children.Add(Ui.Line());
        foreach (var i in s.Items) { list.Children.Add(Row(i).Pad(0, 20, 0, 20)); list.Children.Add(Ui.Line()); }
    }

    private StackPanel Row(Item i)
    {
        var s = Store.Current;
        var busy = s.Measuring.Contains(i.Key);
        var measure = Ui.Outline(busy ? "Measuring" : i.Measured is null ? "Measure real speed" : "Measure again", () => s.Measure(i.Key));
        measure.IsEnabled = !busy;
        var buttons = !i.Ready ? Ui.Row() : Ui.Row(
            i.Runnable ? Ui.Outline("Try it", () => s.OpenChat(i.Key)) : null,
            i.Runnable ? measure.Pad(8) : null,
            Ui.Outline("Show in Explorer", () => s.Reveal(i.Key)).Pad(8),
            Ui.Outline("Delete", () =>
            {
                if (MessageBox.Show($"Delete {i.Name}? This frees {Show.Size(i.Total)}. You can download it again.", "RunIt", MessageBoxButton.OKCancel, MessageBoxImage.None, MessageBoxResult.Cancel) == MessageBoxResult.OK) s.Act("remove", i.Key);
            }).Pad(8));
        var row = Ui.Stack(Ui.Ends(Ui.Stack(Ui.Head(i.Name, 26), Ui.Soft($"{i.Repo}, {i.Label}, {Show.Size(i.Total)}").OneLine().Pad(0, 3)).Pad(0, 0, 12), buttons.Mid()));
        if (!i.Ready) { var p = new ItemProgress(i).Pad(0, 14); live[i.Key] = p; row.Children.Add(p); }
        else if (i.Prediction is not null || i.Measured is not null) row.Children.Add(Versus(i, busy).Pad(0, 14));
        return row;
    }

    /// Predicted against real.
    private static StackPanel Versus(Item i, bool busy)
    {
        var top = Math.Max(1, Math.Max(i.Prediction?.TokensPerS ?? 0, i.Measured?.TokensPerS ?? 0));
        StackPanel Bar(string k, double v, Brush c)
        {
            var b = new Border { Width = Math.Max(2, 320 * v / top), Height = 14, Background = c, CornerRadius = new(2) }.Left().Mid();
            var t = Ui.Soft(k).Mid();
            t.Width = 84;
            return Ui.Row(t, new Grid { Width = 332, Children = { b } }, Ui.Fig($"{Show.Speed(v)} tok/s", 12.5).Mid()).Pad(0, 0, 0, 7);
        }
        var note = i.Measured?.Conditions is { } c ? c.Hot ? "Measured while this PC was hot and holding back. Let it cool and measure again." : c.OnBattery ? "Measured on battery." : null : null;
        return Ui.Stack(
            i.Prediction is { } p ? Bar("Predicted", p.TokensPerS, Theme.Rule) : null,
            i.Measured is { } m ? Bar("Measured", m.TokensPerS, Theme.Marker) : null,
            busy ? Ui.Soft("Timing it on this PC") : note is null ? null : Ui.Soft(note));
    }
}

public sealed class ThisPc : Grid
{
    public ThisPc()
    {
        var s = Store.Current;
        if (s.Report is not { } r) return;
        var main = r.Machine.Gpu is { Integrated: false } g ? g.Name : r.Machine.Cpu;
        var list = new StackPanel { Width = 760, Margin = new(40, 34, 40, 40), HorizontalAlignment = HorizontalAlignment.Left };
        void Add(UIElement e) => list.Children.Add(e);
        Add(new Typed(Ui.Head($"{main} reads {(int)(r.BytesPerS / 1e9)} GB of model a second.", 46), 60, 0, false));
        Add(Ui.Soft("Writing a token means reading the model's weights once, so that figure sets every speed on these screens.", 15).Max(520).Pad(0, 12, 0, 34));
        Add(Ui.Line());
        foreach (var (k, v) in new[]
        {
            ("Processor", $"{r.Machine.Cpu}, {r.Machine.Cores} cores"), ("Graphics", r.Machine.Gpu?.Name ?? "none the engine can use"),
            ("Memory", Show.Memory(r.Machine.RamBytes)), ("Speed kept after 20 s of work", $"{(int)(r.Calibration.Sustained * 100)}%"), ("System", r.Machine.Os),
            ("Tested", $"{DateTimeOffset.FromUnixTimeSeconds((long)r.TakenAt).LocalDateTime:g}, {(int)r.Seconds} s"),
        }) { Add(Ui.Ends(Ui.Soft(k, 13.5), Ui.Body(v)).Pad(0, 9, 0, 9)); Add(Ui.Line()); }
        Add(Ui.Head("What was timed", 26).Pad(0, 44, 0, 12));
        Add(Ui.Line());
        foreach (var x in r.Runs)
        {
            TextBlock Cell(string t, double w, Brush? c = null) { var b = Ui.Fig(t, 13, c).Mid(); b.Width = w; b.TextAlignment = TextAlignment.Right; return b; }
            Add(Ui.Ends(Ui.Row(Ui.Body($"{Show.Size(x.ModelBytes)} test model"), Ui.Soft(x.Place == "gpu" ? "graphics" : "processor", 13.5).Pad(8)), Ui.Row(Cell($"{Show.Speed(x.Decode)} tok/s", 110), Cell($"reads {Show.Speed(x.Prompt)}", 120, Theme.Faint))).Pad(0, 9, 0, 9));
            Add(Ui.Line());
        }
        Add(Ui.Row(Ui.Solid("Test again", s.Test), Ui.Outline("Choose models folder", () =>
        {
            var d = new OpenFolderDialog { Title = "Choose where models go" };
            if (d.ShowDialog() == true) s.MoveModels(d.FolderName);
        }).Mid().Pad(10)).Pad(0, 30));
        Add(Ui.Fig(s.ModelsDir, 12, Theme.Faint).Pad(0, 10));
        Add(Ui.Text($"Models and files: Hugging Bay. Ratings: LMArena leaderboard dataset, CC BY 4.0, {s.RatingsDate}. Engine: llama.cpp. RunIt {s.Version} sends nothing about this computer anywhere.", Theme.Sans, 12.5, Theme.Faint, line: 18).Max(560).Pad(0, 40));
        Children.Add(Ui.Scroll(list));
    }
}

public sealed class Chat : DockPanel
{
    private readonly StackPanel turns = new() { MaxWidth = 660, HorizontalAlignment = HorizontalAlignment.Left, Margin = new(40, 26, 40, 26) };
    private readonly ScrollViewer scroll;
    private readonly TextBlock speed = Ui.Head("", 36), versus = Ui.Soft("", 11.5);
    private readonly TextBox box;
    private readonly Border send = new();
    private TextBlock? reply;

    public Chat()
    {
        var s = Store.Current;
        var item = s.Items.FirstOrDefault(i => i.Key == s.ChatKey);
        var close = Ui.Outline("Close", s.CloseChat);
        close.IsCancel = true;
        var head = Ui.Ends(
            Ui.Stack(Ui.Head(item?.Name ?? "", 26), Ui.Soft("Running on this PC. Nothing leaves it.").Pad(0, 3)),
            Ui.Row(speed.Mid(), Ui.Stack(versus, Ui.Soft(item?.Prediction is { } p ? $"{Show.Speed(p.TokensPerS)} predicted" : "", 11.5)).Mid().Pad(8, 0, 18), close.Mid())).Pad(40, 34, 40, 16);
        var top = Ui.Stack(head, Ui.Line());
        SetDock(top, Dock.Top);
        box = new TextBox { Style = (Style)Application.Current.Resources["Field"], FontFamily = Theme.Sans, FontSize = 15, TextWrapping = TextWrapping.Wrap, MaxLines = 6, VerticalAlignment = VerticalAlignment.Center, Margin = new(0, 4, 10, 4) };
        System.Windows.Automation.AutomationProperties.SetName(box, "Message");
        box.PreviewKeyDown += (_, e) => { if (e.Key == Key.Enter && Keyboard.Modifiers != ModifierKeys.Shift) { e.Handled = true; Send(); } };
        var bar = new Border { Background = Theme.Sheet, BorderBrush = Theme.Rule, BorderThickness = new(1), CornerRadius = new(8), Padding = new(16, 8, 8, 8), MinHeight = 58, Margin = new(40, 0, 40, 26), Child = Ui.Ends(box, send) };
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
        if (s.ChatLoading) turns.Children.Add(new Typed(Ui.Head("Loading it into memory.", 26, Theme.Faint), 30));
        else if (s.Turns.Count == 0 && s.ChatError is null) turns.Children.Add(Ui.Head("Ask it anything.", 26, Theme.Faint));
        foreach (var t in s.Turns)
        {
            if (t.Mine) { turns.Children.Add(Ui.Body(t.Text, 15, bold: true).Pad(0, 0, 0, 14)); continue; }
            if (t.Thought != "")
            {
                var thought = Ui.Soft(t.Thought, 13).Pad(0, 4, 0, 10);
                thought.Visibility = Visibility.Collapsed;
                turns.Children.Add(Ui.Outline("Thinking", () => thought.Visibility = thought.Visibility == Visibility.Visible ? Visibility.Collapsed : Visibility.Visible).Pad(0, 0, 0, 8));
                turns.Children.Add(thought);
            }
            reply = Ui.Text(t.Text, Theme.Reading, 17, line: 26).Pad(0, 0, 0, 22);
            turns.Children.Add(reply);
        }
        if (s.ChatNote is { } n && !s.ChatBusy) turns.Children.Add(Ui.Soft(n));
        if (s.ChatError is { } e) turns.Children.Add(Ui.Body(e, 14, Theme.Alarm));
        send.Child = s.ChatBusy ? Ui.Outline("Stop", s.StopReply).Mid() : Ui.Solid("Send", Send).Mid();
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
        if (s.ChatBusy) reply.Inlines.Add(new System.Windows.Documents.Run("  ") { Background = Theme.Marker });
        scroll.ScrollToBottom();
    }
}
