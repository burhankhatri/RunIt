using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace RunIt;

/// One model: how it will feel, whether it fits, and what to get.
public sealed class ModelPage : Grid
{
    private readonly Opened opened;
    private readonly Border hero = new() { CornerRadius = new(18), Padding = new(28), Height = 316 }, facts = new() { CornerRadius = new(14), Padding = new(20), Background = Theme.Card }, sizes = new() { CornerRadius = new(14), Padding = new(20), Background = Theme.Card };
    private Detail? detail;
    private string? error, label;
    private string heroState = "", factState = "";
    private bool showOthers;
    private ItemProgress? progress;

    public ModelPage(Opened opened)
    {
        (this.opened, label) = (opened, opened.Seed?.Variant.Label);
        Background = Theme.Ground;
        var back = Ui.Plain("←  Back", () => Store.Current.Open(null));
        back.IsCancel = true;
        Children.Add(Ui.Page(Ui.Stack(back, hero.Pad(0, 18), Ui.Columns([(facts.Top(), 1), (sizes.Top(), 1)], 16).Pad(0, 18)), 1080));
        Build();
        this.Listen(t => { if (t is "items") Facts(); });
        Load();
    }

    private Choice? Seed => opened.Seed;
    private string Model => Seed?.Name ?? detail?.Name ?? opened.Name;
    private SolidColorBrush Tint => Store.Current.Tint(Model);
    private Offer? Chosen => detail?.Variants.FirstOrDefault(v => v.Variant.Label == label) ?? detail?.Variants.FirstOrDefault() ?? (Seed is null ? null : new Offer(Seed.Variant, Seed.Prediction));

    private async void Load()
    {
        error = null;
        try
        {
            detail = await Store.Current.Core!.Call<Detail>("model", new { id = opened.Id });
            label ??= (detail.Variants.FirstOrDefault(v => v.Variant.Label == "Q4_K_M") ?? detail.Variants.FirstOrDefault())?.Variant.Label;
        }
        catch (Exception e) { error = e.Message; }
        Build();
    }

    private void Build() { Hero(); factState = ""; Facts(); Sizes(); }

    private void Hero()
    {
        var o = Chosen;
        var p = o?.Prediction;
        // Keep the sample running when only the details below change.
        var state = $"{Model}{p?.TokensPerS}{p?.Tier}{o?.Variant.Label}{detail is null}";
        if (state == heroState) return;
        heroState = state;
        var tint = Tint;
        hero.Background = tint;
        var title = Ui.Stack(Ui.Bold(Model, 34, Theme.Ink), Ui.Body(string.Join(", ", new[] { detail?.Repo ?? Seed?.Repo, (detail?.License ?? Seed?.License) is { } l ? Show.License(l) : null }.OfType<string>()), 13, Theme.InkSoft).OneLine().Pad(0, 5));
        UIElement? speed = p is null ? null : p.Tier == "too_big" ? Ui.Pixel("Too big for this PC", 52, Theme.Ink).Foot().Left()
            : Ui.Row(new Figure(p.TokensPerS, 128), Ui.Stack(Ui.Bold("tokens a second", 14, Theme.Ink), Ui.Body($"{(p.Rough ? "rough, " : "")}{Show.Speed(p.TokensPerSLow)} to {Show.Speed(p.TokensPerSHigh)}", 13, Theme.InkSoft).Pad(0, 2)).Foot().Pad(12, 0, 0, 14)).Foot().Left();
        var left = new Grid { Children = { title.Top() } };
        if (speed is not null) left.Children.Add(speed);
        var show = Ui.Row(p is { Tier: not "too_big" } ? new SpeedPreview(p.TokensPerS, opened.Code, 40).Wide(230).Top().Pad(0, 0, 22) : null, new Creature(Model, 150).Top()).Right().Top();
        var right = new Grid { Children = { show } };
        if (o is not null) right.Children.Add(new Get(opened.Id, detail?.Repo ?? Seed?.Repo ?? "", o.Variant, Theme.Ink, tint, true, detail?.Variants.Any(v => v.Variant.Label == o.Variant.Label) ?? false).Right().Foot());
        hero.Child = Ui.Columns([(left, 0), (right, -1)], 24);
    }

    private void Facts()
    {
        var s = Store.Current;
        var o = Chosen;
        var item = o is null ? null : s.Find(detail?.Repo ?? Seed?.Repo ?? "", o.Variant.Files);
        var state = $"{item?.Status}{o?.Variant.Label}";
        if (state == factState && factState != "") { if (item is { Ready: false }) progress?.Show(item); return; }
        factState = state;
        progress = null;
        var list = new StackPanel();
        void Add(UIElement e) => list.Children.Add(e);
        void Fact(string k, string v) => Add(Ui.Ends(Ui.Soft(k, 13.5), Ui.Bold(v, 13.5)).Pad(0, 0, 0, 12));
        if (item is { Ready: true }) Add(Ui.Row(Ui.Plain("Show in Explorer", () => s.Reveal(item.Key)), Ui.Plain("Open in Library", () => s.Go(Place.Library)).Pad(8)).Pad(0, 0, 0, 16));
        else if (item is not null) { progress = new ItemProgress(item).Pad(0, 0, 0, 16); Add(progress); }
        if (o?.Prediction is { } p)
        {
            Add(Ui.Bold(p.Tier switch
            {
                "comfortable" => "Fits with room to spare",
                "tight" => "Fits, but close heavy apps first",
                "partial" => $"{(int)(p.GpuShare * 100)}% runs on graphics, the rest on the processor",
                _ => "Needs more memory than this PC has",
            }, 15));
            Add(new Blocks(p.NeedBytes / Math.Max(p.CapacityBytes, 1), 28, 9, 18, p.Tier == "comfortable" ? Tint : Theme.Alarm).Pad(0, 14, 0, 14));
            Fact("Memory", $"{Show.Memory(p.NeedBytes)} of {Show.Memory(p.CapacityBytes)}");
            if (p.Tier != "too_big")
            {
                Fact("Reads your prompt at", $"{Show.Speed(p.PromptTokensPerS)} tokens/s");
                Fact("In a long conversation", $"{Show.Speed(p.TokensPerSDeep)} tokens/s");
                if (p.TokensPerSSustained < p.TokensPerS * 0.95) Fact("After 20 s of steady work", $"{Show.Speed(p.TokensPerSSustained)} tokens/s");
            }
            else Add(Ui.Soft("You can still download it for another computer.", 13).Pad(0, 0, 0, 12));
        }
        else if (o is not null && detail?.Capacity is { } cap)
        {
            Add(Ui.Bold(o.Variant.Size < cap * 0.8 ? "Fits in this PC's model memory" : "Larger than the model memory here", 15));
            Add(Ui.Soft("RunIt predicts speed for language models only.", 13).Pad(0, 6, 0, 12));
        }
        if ((detail?.Rating ?? Seed?.Rating) is { } r)
        {
            Fact("LMArena rating", $"{r.Score}");
            Add(Ui.Text($"From {Show.Count(r.Votes)} blind votes, for the full model. Smaller files lose a little.", Theme.Sans, 12.5, Theme.Soft, line: 18));
        }
        if (error is not null) { Add(Ui.Body(error, 13.5, Theme.Alarm).Pad(0, 8, 0, 10)); Add(Ui.Plain("Try again", Load)); }
        if (list.Children.Count == 0) Add(Ui.Soft("Nothing to size up yet.", 13.5));
        facts.Child = list;
    }

    private void Sizes()
    {
        var list = new StackPanel();
        void Add(UIElement e) => list.Children.Add(e);
        sizes.Child = list;
        if (detail is not { } d)
        {
            if (error is null) Add(Ui.Row(new Creature(Model, 20, Theme.Soft, 0.2).Mid(), Ui.Soft("Checking what can be downloaded", 13.5).Mid().Pad(12)));
            return;
        }
        if (d.Variants.Count == 0 && d.Others.Count == 0) Add(Ui.Soft(d.Note ?? "Hugging Bay does not host files for this one yet.", 13.5));
        if (d.Variants.Count > 0)
        {
            Add(Ui.Bold("Sizes", 15));
            Add(Ui.Soft("Larger is closer to the original, and slower.").Pad(0, 3, 0, 10));
            foreach (var o in d.Variants)
            {
                var on = o.Variant.Label == Chosen?.Variant.Label;
                var dot = new System.Windows.Shapes.Rectangle { Width = 10, Height = 10, Fill = on ? Tint : Theme.Off, SnapsToDevicePixels = true };
                var row = new Border { Height = 36, CornerRadius = new(8), Padding = new(8, 0, 8, 0), Background = on ? Theme.Raised : Brushes.Transparent, Child = Ui.Columns([(dot.Mid(), -1), (Ui.Text(o.Variant.Label, Theme.Sans, 13.5, bold: on).Mid(), 0), (Ui.Soft(Show.Size(o.Variant.Size), 13).Mid(), -1), (new Border { Width = 108, Child = Rows.Speed(o.Prediction) }.Mid(), -1)], 10) };
                Add(Ui.Bare(row, () => { label = o.Variant.Label; Build(); }, o.Variant.Label));
            }
        }
        if (d.Others.Count == 0) return;
        var open = showOthers || d.Variants.Count == 0;
        var button = d.Variants.Count == 0 ? Ui.Block($"Get all, {Show.Size(d.Others.Sum(f => f.Size))}", () => Store.Current.Download(d.Id, d.Others), Tint)
            : Ui.Plain(open ? "Hide" : $"Show {d.Others.Count}", () => { showOthers = !showOthers; Sizes(); });
        Add(Ui.Ends(Ui.Bold(d.Variants.Count == 0 ? "Files" : "Other files", 15).Mid(), button).Pad(0, d.Variants.Count == 0 ? 0 : 18, 0, 8));
        if (!open) return;
        foreach (var f in d.Others)
        {
            var have = Store.Current.Find(d.Repo, [f]);
            UIElement get = have is null ? Ui.Plain("Get", () => Store.Current.Download(d.Id, [f])) : Ui.Bold(have.Ready ? "saved" : $"{(int)(have.Done / Math.Max(have.Total, 1) * 100)}%", 12, Tint);
            Add(Ui.Columns([(Ui.Text(f.Path, Theme.Mono, 12).OneLine().Mid(), 0), (Ui.Soft(Show.Size(f.Size), 12).Mid(), -1), (get, -1)], 10).Pad(0, 4));
        }
    }
}

public sealed class ItemProgress : StackPanel
{
    private readonly Blocks blocks;
    private readonly TextBlock line;

    public ItemProgress(Item i)
    {
        var bad = i.Status == "failed";
        blocks = new Blocks(0, 28, 9, 18, bad ? Theme.Alarm : Store.Current.Tint(i.Name));
        line = Ui.Body("", 12.5, bad ? Theme.Alarm : Theme.Soft).Mid();
        var s = Store.Current;
        var buttons = Ui.Row(
            i.Status switch
            {
                "active" or "queued" => Ui.Plain("Pause", () => s.Act("pause", i.Key)),
                "paused" => Ui.Plain("Resume", () => s.Act("resume", i.Key)),
                _ => Ui.Plain("Try again", () => s.Act("resume", i.Key)),
            },
            Ui.Plain(bad ? "Remove" : "Cancel", () => s.Act("remove", i.Key)).Pad(8));
        Children.Add(blocks);
        Children.Add(Ui.Ends(line.Pad(0, 0, 10), buttons).Pad(0, 10));
        Show(i);
    }

    public void Show(Item i)
    {
        blocks.Set(i.Done / Math.Max(i.Total, 1));
        line.Text = i.Status switch
        {
            "queued" => "Waiting",
            "failed" => i.Error ?? "Failed",
            "paused" => $"Paused at {RunIt.Show.Size(i.Done)} of {RunIt.Show.Size(i.Total)}",
            _ => i.Done >= i.Total ? "Checking" : $"{RunIt.Show.Size(i.Done)} of {RunIt.Show.Size(i.Total)}, {RunIt.Show.Size(i.BytesPerS)}/s  {RunIt.Show.Left(i)}",
        };
    }
}
