using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace RunIt;

public sealed class ForYou : Grid
{
    private static string? selected;
    private readonly StackPanel list = new() { MaxWidth = 720, HorizontalAlignment = HorizontalAlignment.Left };
    private readonly Border pane = new();
    private readonly List<Action> redraw = [];

    public ForYou()
    {
        Build();
        this.Listen(t => { if (t is "shortlist") Build(); else if (t is "catalog" && Store.Current.Shortlist is null) Build(); });
    }

    private void Build()
    {
        Children.Clear();
        ColumnDefinitions.Clear();
        redraw.Clear();
        var s = Store.Current;
        if (s.Shortlist is not { } list_)
        {
            if (s.ShortlistError is { } e)
                Children.Add(Ui.Stack(Ui.Head("The catalog is out of reach.", 40), Ui.Soft(e, 15).Pad(0, 14), Ui.Solid("Try again", async () => await s.Recommend()).Pad(0, 18)).Max(520).Mid().Pad(40));
            else
            {
                var bar = Ui.Bar(2, Theme.Ink, false);
                bar.Set(s.CatalogProgress ?? 0);
                Children.Add(Ui.Stack(Ui.Head("Reading the catalog.", 40), bar.View.Pad(0, 20, 0, 14).Max(320), Ui.Soft("Sizing up every model hosted on Hugging Bay.", 14)).Mid().Pad(40));
                bar.View.Width = 320;
            }
            return;
        }
        var all = list_.Picks.Select(p => p.Choice).Concat(list_.Ranked).Concat(list_.Unrated).ToList();
        if (all.All(c => c.Id != selected)) selected = all.FirstOrDefault()?.Id;
        ColumnDefinitions.Add(new());
        ColumnDefinitions.Add(new() { Width = new(1) });
        ColumnDefinitions.Add(new() { Width = new(410) });

        list.Children.Clear();
        if (list_.Picks.Count > 0) Verdict(list_.Picks[0]);
        else { list.Children.Add(Ui.Head("Nothing rated fits this computer comfortably.", 40)); list.Children.Add(Ui.Soft("Smaller models below may still be worth a try.", 15).Pad(0, 10)); }

        var picks = new StackPanel { Margin = new(0, 34, 0, 0) };
        picks.Children.Add(Ui.Line());
        for (var n = 0; n < list_.Picks.Count; n++) { picks.Children.Add(Slot(PickRow, list_.Picks[n]).Rise(0.55 + n * 0.09)); picks.Children.Add(Ui.Line()); }
        list.Children.Add(picks);
        if (list_.Ranked.Count > 0) Table("Rated, and runs here", $"Rating: LMArena, {list_.RatingsDate}. Higher is better.", list_.Ranked.Take(14));
        if (list_.Unrated.Count > 0) Table("Popular, not rated yet", "From the model's own publisher, by downloads.", list_.Unrated);

        (list.Parent as ContentControl)?.SetCurrentValue(ContentControl.ContentProperty, null);
        list.Margin = new(40, 34, 40, 40);
        Children.Add(Ui.Scroll(list));
        var line = Ui.Tall();
        SetColumn(line, 1);
        SetColumn(pane, 2);
        Children.Add(line); Children.Add(pane);
        Pane();
    }

    private void Pane()
    {
        var s = Store.Current.Shortlist!;
        var c = s.Picks.Select(p => p.Choice).Concat(s.Ranked).Concat(s.Unrated).FirstOrDefault(c => c.Id == selected);
        pane.Child = c is null ? null : new DetailPane(c.Id, c, s.Picks.Any(p => p.Choice.Id == c.Id && p.Role == "code"));
    }

    private void Select(string id) { if (selected == id) return; selected = id; foreach (var r in redraw) r(); Pane(); }

    /// A row that can redraw itself when the selection moves.
    private Border Slot<T>(Func<T, UIElement> make, T data)
    {
        var b = new Border { Child = make(data) };
        redraw.Add(() => b.Child = make(data));
        return b;
    }

    private void Verdict(Pick best)
    {
        var chip = Store.Current.Report?.Machine is { } m ? m.Gpu is { Integrated: false } g ? g.Name : m.Cpu : "computer";
        list.Children.Add(new Typed(Ui.Head($"Your {chip} is at its best with", 44), 50, 0, false));
        list.Children.Add(new Marked(best.Choice.Name, 52, 0.7));
        var p = best.Choice.Prediction;
        var text = $"It should write {Show.Speed(p.TokensPerSLow)} to {Show.Speed(p.TokensPerSHigh)} tokens a second and leave {Show.Memory(Math.Max(0, p.CapacityBytes - p.NeedBytes))} of model memory free.";
        var names = new Dictionary<string, string> { ["quick"] = "the quickest", ["code"] = "best for code", ["smart"] = "the smartest it can hold" };
        var also = best.Also.Where(names.ContainsKey).Select(a => names[a]).ToList();
        if (also.Count > 0) text += $" Also {string.Join(" and ", also)}.";
        list.Children.Add(Ui.Text(text, Theme.Sans, 16, Theme.Faint, line: 24).Max(520).Pad(0, 12));
    }

    private UIElement PickRow(Pick p)
    {
        var words = Ui.Stack(Ui.Soft(p.Title), Ui.Head(p.Choice.Name, 27).Pad(0, 4, 0, 4), Ui.Soft(p.Why, 13.5)).Pad(0, 0, 18);
        var speed = Ui.Stack(new Rolling(p.Choice.Prediction.TokensPerS, 38).Right(), Ui.Soft("tokens/s", 11.5).Right());
        return Ui.Choice(Ui.Ends(words, speed), selected == p.Choice.Id, () => Select(p.Choice.Id), $"{p.Title}: {p.Choice.Name}", new(16, 16, 14, 16));
    }

    private void Table(string title, string sub, IEnumerable<Choice> rows)
    {
        list.Children.Add(Ui.Head(title, 26).Pad(0, 46));
        list.Children.Add(Ui.Soft(sub).Pad(0, 4, 0, 14));
        list.Children.Add(Ui.Line());
        foreach (var c in rows)
        {
            list.Children.Add(Slot(x => Rows.Model(x.Name, $"{x.Variant.Label}, {Show.Size(x.Variant.Size)}", x.Rating?.Score, x.Prediction, selected == x.Id, () => Select(x.Id)), c));
            list.Children.Add(Ui.Line());
        }
    }
}

public static class Rows
{
    public static UIElement Model(string name, string sub, int? rating, Prediction? p, bool on, Action act)
    {
        var words = Ui.Stack(Ui.Body(name, 14, bold: true).OneLine(), Ui.Soft(sub, 12).OneLine().Pad(0, 2));
        var score = Ui.Fig(rating?.ToString() ?? "", 13, Theme.Faint).Right().Mid();
        score.Width = 48; score.TextAlignment = TextAlignment.Right;
        var speed = new Border { Width = 116, Child = Speed(p) }.Mid();
        return Ui.Choice(Ui.Ends(words.Pad(0, 0, 10), Ui.Row(score, speed)), on, act, name, new(16, 9, 12, 9));
    }

    public static UIElement? Speed(Prediction? p)
    {
        if (p is null) return null;
        if (p.Tier == "too_big") return Ui.Soft("too big", 13).Right();
        var row = Ui.Row(
            p.Tier == "comfortable" ? null : Ui.Body(p.Tier == "tight" ? "tight" : "split", 11, Theme.Alarm).Pad(0, 4, 5),
            Ui.Fig((p.Rough ? "~" : "") + Show.Speed(p.TokensPerS), 15, bold: true),
            Ui.Soft("tok/s", 11).Pad(4, 4));
        return row.Right();
    }
}
