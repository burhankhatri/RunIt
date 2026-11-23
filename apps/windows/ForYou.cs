using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace RunIt;

public sealed class ForYou : Grid
{
    private Blocks? loading;

    public ForYou()
    {
        Build();
        this.Listen(t => { if (t is "shortlist") Build(); else if (t is "catalog") loading?.Set(Store.Current.CatalogProgress ?? 0); });
    }

    private void Build()
    {
        Children.Clear();
        loading = null;
        var s = Store.Current;
        if (s.Shortlist is { } list) { Children.Add(Ui.Page(Page(list))); return; }
        if (s.ShortlistError is { } e)
        {
            Children.Add(Ui.Stack(new Creature("offline", 100, Theme.Alarm).Left(), Ui.Bold("The catalog is out of reach", 26).Pad(0, 16), Ui.Soft(e, 15).Pad(0, 12), Ui.Block("Try again", async () => await s.Recommend()).Pad(0, 18)).Max(480).Centre().Mid());
            return;
        }
        loading = new Blocks(s.CatalogProgress ?? 0, 30).Centre();
        Children.Add(Ui.Stack(new Creature(s.Report?.Machine.Cpu ?? "RunIt", 120, Theme.Orange, 0.2).Centre(), Ui.Bold("Sizing up every model on Hugging Bay", 18).Centre().Pad(0, 22, 0, 22), loading).Centre().Mid());
    }

    /// Up to three cards: the picks, then the next best so there is always a choice.
    private static List<(Choice Choice, string Line, bool Code)> Cards(Shortlist s)
    {
        var list = s.Picks.Take(3).Select(p =>
        {
            var x = p.Choice.Prediction;
            var line = p.Role switch
            {
                "best" => x.TokensPerS >= 10 ? $"Best for this PC. {Show.Memory(Math.Max(0, x.CapacityBytes - x.NeedBytes))} to spare." : "The best this PC can run, slowly.",
                "quick" => "Quickest worth using.",
                "code" => "Best for code.",
                _ => x.Tier == "tight" ? "Smartest it can hold. A tight fit." : "Smartest it can hold.",
            };
            return (p.Choice, line, p.Role == "code");
        }).ToList();
        foreach (var c in s.Ranked.Concat(s.Unrated))
            if (list.Count < 3 && list.All(x => x.Choice.Id != c.Id)) list.Add((c, c.Rating is { } r ? $"Rated {r.Score}. Runs well too." : "Popular, not rated yet.", false));
        return list;
    }

    private static StackPanel Page(Shortlist s)
    {
        var r = Store.Current.Report;
        var cards = Cards(s);
        var shown = cards.Select(c => c.Choice.Id).ToHashSet();
        var page = Ui.Stack(
            Ui.Bold(s.Picks.Count == 0 ? "Nothing rated fits comfortably" : "Your PC runs these well", 27),
            Ui.Soft(r is null ? "" : $"{(r.Machine.Gpu is { Integrated: false } g ? g.Name : r.Machine.Cpu)}, {Math.Round(r.Machine.RamBytes / 1073741824)} GB. Tested at {(int)(r.BytesPerS / 1e9)} GB of model a second.", 14).Pad(0, 6, 0, 20));
        if (cards.Count > 0) page.Children.Add(Ui.Columns([.. cards.Select((c, n) => ((UIElement)Card(c.Choice, c.Line, c.Code, n == 0).Rise(0.05 + n * 0.08), n == 0 ? 1.55 : 1.0))], 14));
        void Table(string title, string sub, IEnumerable<Choice> rows)
        {
            var list = rows.Where(c => !shown.Contains(c.Id)).Take(12).ToList();
            if (list.Count == 0) return;
            page.Children.Add(Ui.Ends(Ui.Bold(title, 16), Ui.Soft(sub).Foot()).Pad(0, 32, 0, 10));
            foreach (var c in list)
            {
                page.Children.Add(Ui.Line());
                page.Children.Add(Rows.Model(c.Name, Show.Size(c.Variant.Size), c.Rating?.Score, c.Prediction, () => Store.Current.Open(new(c.Id, c.Name, c))));
            }
        }
        Table("More that run here", $"LMArena rating, {s.RatingsDate}", s.Ranked);
        Table("Popular, not rated yet", "From the model's own publisher", s.Unrated);
        return page;
    }

    private static FrameworkElement Card(Choice c, string line, bool code, bool big)
    {
        var tint = Store.Current.Tint(c.Name);
        var creature = new Creature(c.Name, 110);
        var number = Ui.Row(new Figure(c.Prediction.TokensPerS, 64), Ui.Bold("tokens/s", 12.5, Theme.Ink).Foot().Pad(7, 0, 0, 9)).Foot().Left().Pad(20, 0, 0, 8);
        UIElement upper = big
            ? Ui.Columns([(creature.Top().Pad(0, 2), -1), (new SpeedPreview(c.Prediction.TokensPerS, code, 22).Top(), 0)], 24).Pad(26, 22, 16)
            : creature.Centre().Top().Pad(0, 24);
        var art = new Border { Height = 224, Background = tint, CornerRadius = new(14, 14, 0, 0), Child = new Grid { Children = { upper, number } } };
        var words = Ui.Stack(Ui.Bold(c.Name, 16).OneLine(), Ui.Soft(line, 13).Pad(0, 4)).Mid();
        var below = new Border
        {
            Background = Theme.Card, CornerRadius = new(0, 0, 14, 14), Padding = new(18, 14, 18, 14), MinHeight = 78,
            Child = big ? Ui.Ends(words.Pad(0, 0, 12), new Get(c.Id, c.Repo, c.Variant, tint, Theme.Ink, false).Mid()) : words,
        };
        var grid = new Grid { Children = { art, below }, Background = Brushes.Transparent, Cursor = System.Windows.Input.Cursors.Hand };
        grid.RowDefinitions.Add(new() { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new());
        SetRow(below, 1);
        Ui.Hop(grid, creature, 8);
        grid.MouseLeftButtonUp += (_, _) => Store.Current.Open(new(c.Id, c.Name, c, code));
        grid.Focusable = true;
        grid.FocusVisualStyle = null;
        grid.KeyDown += (_, e) => { if (e.Key is System.Windows.Input.Key.Enter or System.Windows.Input.Key.Space && e.OriginalSource == grid) Store.Current.Open(new(c.Id, c.Name, c, code)); };
        System.Windows.Automation.AutomationProperties.SetName(grid, $"{c.Name}, {Show.Speed(c.Prediction.TokensPerS)} tokens a second. {line}");
        return grid;
    }
}

/// Download, progress, or try it: whatever comes next for this file.
public sealed class Get : Border
{
    private string state = "";
    public Get(string id, string repo, Variant v, Brush fill, Brush label, bool tall, bool enabled = true)
    {
        void Draw()
        {
            var s = Store.Current;
            var i = s.Find(repo, v.Files);
            var (text, act, on) = i switch
            {
                null => ($"Get {Show.Size(v.Size)}", () => s.Download(id, v.Files), enabled),
                { Ready: true } => ("Try it", () => s.OpenChat(i.Key), i.Runnable),
                { Status: "paused" } => ("Resume", () => s.Act("resume", i.Key), true),
                { Status: "failed" } => ("Try again", () => s.Act("resume", i.Key), true),
                _ => ($"{(int)(i.Done / Math.Max(i.Total, 1) * 100)}%", (Action)(() => s.Go(Place.Library)), true),
            };
            if (state == text + on) return;
            state = text + on;
            var b = Ui.Block(text, act, fill, label, tall);
            b.IsEnabled = on;
            Child = b;
        }
        Draw();
        this.Listen(t => { if (t is "items") Draw(); });
    }
}

public static class Rows
{
    public static UIElement Model(string name, string sub, int? rating, Prediction? p, Action act)
    {
        var tile = Tile.Of(name, 34).Mid();
        var score = Ui.Soft(rating?.ToString() ?? "", 13).Mid().Wide(52);
        score.TextAlignment = TextAlignment.Right;
        var speed = new Border { Width = 112, Child = Speed(p) }.Mid();
        FrameworkElement blocks = p is { Tier: not "too_big" } ? new Blocks(p.TokensPerS / 60, color: Store.Current.Tint(name)) : new Border { Width = 285 };
        var words = Ui.Stack(Ui.Bold(name).OneLine(), Ui.Soft(sub, 12).OneLine().Pad(0, 2)).Mid();
        var row = new Border { Height = 54, Padding = new(6, 0, 6, 0), CornerRadius = new(8), Background = Brushes.Transparent, Child = Ui.Columns([(tile, -1), (words, 0), (blocks.Mid(), -1), (score, -1), (speed, -1)], 14) };
        row.MouseEnter += (_, _) => row.Background = Theme.Card;
        row.MouseLeave += (_, _) => row.Background = Brushes.Transparent;
        Ui.Hop(row, tile, 3);
        return Ui.Bare(row, act, name);
    }

    public static UIElement? Speed(Prediction? p)
    {
        if (p is null) return null;
        if (p.Tier == "too_big") return Ui.Soft("too big", 13).Right();
        return Ui.Row(
            p.Tier == "comfortable" ? null : Ui.Bold(p.Tier == "tight" ? "tight" : "split", 11, Theme.Alarm).Foot().Pad(0, 0, 6, 5),
            Ui.Pixel((p.Rough ? "~" : "") + Show.Speed(p.TokensPerS), 25),
            Ui.Soft("tok/s", 11).Foot().Pad(5, 0, 0, 5)).Right();
    }
}
