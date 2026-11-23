using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace RunIt;

/// One model: how it will feel, whether it fits, and what to download.
public sealed class DetailPane : Border
{
    private readonly string id;
    private readonly Choice? seed;
    private readonly bool code;
    private readonly StackPanel body = new() { Margin = new(28, 34, 28, 36) };
    private readonly Border action = new(), feel = new();
    private Detail? detail;
    private string? error, label;
    private string actionState = "";
    private double shownSpeed = -1;

    public DetailPane(string id, Choice? seed, bool code)
    {
        (this.id, this.seed, this.code, label) = (id, seed, code, seed?.Variant.Label);
        Background = Theme.Sheet;
        Child = Ui.Scroll(body);
        Build();
        this.Listen(t => { if (t is "items") Action(); });
        Load();
    }

    private async void Load()
    {
        error = null;
        try
        {
            detail = await Store.Current.Core!.Call<Detail>("model", new { id });
            label ??= (detail.Variants.FirstOrDefault(v => v.Variant.Label == "Q4_K_M") ?? detail.Variants.FirstOrDefault())?.Variant.Label;
        }
        catch (Exception e) { error = e.Message; }
        Build();
    }

    private Offer? Chosen => detail?.Variants.FirstOrDefault(v => v.Variant.Label == label) ?? detail?.Variants.FirstOrDefault() ?? (seed is null ? null : new Offer(seed.Variant, seed.Prediction));

    private void Build()
    {
        (action.Parent as Panel)?.Children.Remove(action);
        (feel.Parent as Panel)?.Children.Remove(feel);
        body.Children.Clear();
        actionState = "";
        void Add(UIElement e) => body.Children.Add(e);

        Add(Ui.Head(seed?.Name ?? detail?.Name ?? " ", 32));
        Add(Ui.Soft(string.Join("  ·  ", new[] { detail?.Repo ?? seed?.Repo, (detail?.License ?? seed?.License) is { } l ? Show.License(l) : null }.OfType<string>())).Pad(0, 6));
        if (Chosen is { } o)
        {
            if (o.Prediction is { } p)
            {
                if (p.Tier != "too_big") { Feel(p); Add(feel); }
                Add(Fit(p).Pad(0, 24));
            }
            else if (detail?.Capacity is { } cap)
                Add(Ui.Soft(o.Variant.Size < cap * 0.8 ? "The file fits in this computer's model memory. RunIt predicts speed for language models only." : "This file is larger than the model memory here.", 13.5).Pad(0, 22));
            Action();
            Add(action);
        }
        if ((detail?.Rating ?? seed?.Rating) is { } r)
            Add(Ui.Text($"Rated {r.Score} from {Show.Count(r.Votes)} blind votes on LMArena. The rating is for the full model; smaller files lose a little.", Theme.Sans, 12.5, Theme.Faint, line: 18).Pad(0, 18));
        if (error is not null) { Add(Ui.Body(error, 13.5, Theme.Alarm).Pad(0, 22, 0, 10)); Add(Ui.Outline("Try again", Load)); }
        else if (detail is null) Add(Ui.Soft("Checking what can be downloaded").Pad(0, 28));
        if (detail is not null) Files(detail);
    }

    private void Feel(Prediction p)
    {
        // Keep the sample running when only the details around it change.
        if (Math.Abs(shownSpeed - p.TokensPerS) < 0.01 && feel.Child is not null) return;
        shownSpeed = p.TokensPerS;
        var side = Ui.Stack(Ui.Body("tokens a second", 13), Ui.Soft($"{(p.Rough ? "rough estimate, " : "")}{Show.Speed(p.TokensPerSLow)} to {Show.Speed(p.TokensPerSHigh)}", 12)).Pad(10, 0, 0, 10);
        side.VerticalAlignment = VerticalAlignment.Bottom;
        feel.Child = Ui.Stack(
            Ui.Row(new Rolling(p.TokensPerS, 72), side).Pad(0, 10),
            Ui.Soft("This is that speed:", 12).Pad(0, 4, 0, 8),
            new Border { Background = Theme.Paper, CornerRadius = new(6), Padding = new(16), Height = 214, ClipToBounds = true, Child = new SpeedPreview(p.TokensPerS, code) });
    }

    private static StackPanel Fit(Prediction p)
    {
        var word = p.Tier switch
        {
            "comfortable" => "Fits with room to spare",
            "tight" => "Fits, but close heavy apps first",
            "partial" => $"Too big for graphics memory, so {(int)(p.GpuShare * 100)}% runs there and the rest on the processor",
            _ => "Too big for this computer",
        };
        var bar = Ui.Bar(8, p.Tier == "comfortable" ? Theme.Ink : Theme.Alarm);
        bar.Set(p.NeedBytes / Math.Max(p.CapacityBytes, 1));
        var s = Ui.Stack(bar.View, Ui.Ends(Ui.Body(word, 13).Pad(0, 0, 10), Ui.Fig($"{Show.Memory(p.NeedBytes)} of {Show.Memory(p.CapacityBytes)}", 12, Theme.Faint)).Pad(0, 8));
        if (p.Tier == "too_big") return s;
        void Fact(string k, string v) => s.Children.Add(Ui.Ends(Ui.Soft(k), Ui.Fig(v, 12.5)).Pad(0, 6));
        s.Children.Add(new Border { Height = 6 });
        Fact("Reads your prompt at", $"{Show.Speed(p.PromptTokensPerS)} tokens/s");
        Fact("In a long conversation", $"{Show.Speed(p.TokensPerSDeep)} tokens/s");
        if (p.TokensPerSSustained < p.TokensPerS * 0.95) Fact("After 20 s of steady work", $"{Show.Speed(p.TokensPerSSustained)} tokens/s");
        return s;
    }

    private void Action()
    {
        if (Chosen is not { } o) return;
        var s = Store.Current;
        var item = s.Find(detail?.Repo ?? seed?.Repo ?? "", o.Variant.Files);
        var state = item is null ? $"none{detail is null}{o.Variant.Label}" : item.Ready ? "ready" : item.Key + item.Status;
        if (item is not null && !item.Ready && state == actionState && action.Child is ItemProgress live) { live.Show(item); return; }
        if (state == actionState) return;
        actionState = state;
        action.Margin = new(0, 24, 0, 0);
        if (item is null)
        {
            var b = Ui.Solid($"Download, {Show.Size(o.Variant.Size)}", () => s.Download(id, o.Variant.Files));
            b.IsEnabled = detail is not null && detail.Variants.Any(v => v.Variant.Label == o.Variant.Label);
            action.Child = Ui.Stack(b, o.Prediction?.Tier == "too_big" ? Ui.Body("This will not fit in memory here. You can still download it for another computer.", 12.5, Theme.Alarm).Pad(0, 8) : null);
        }
        else if (item.Ready) action.Child = Ui.Row(item.Runnable ? Ui.Solid("Try it", () => s.OpenChat(item.Key)).Pad(0, 0, 10) : null, Ui.Outline("Show in Explorer", () => s.Reveal(item.Key)).Mid());
        else action.Child = new ItemProgress(item);
    }

    private void Files(Detail d)
    {
        void Add(UIElement e) => body.Children.Add(e);
        if (d.Variants.Count == 0 && d.Others.Count == 0) Add(Ui.Soft(d.Note ?? "Hugging Bay does not host files for this one yet.", 13.5).Pad(0, 24));
        if (d.Variants.Count > 0)
        {
            Add(Ui.Head("Sizes", 20).Pad(0, 34));
            Add(Ui.Soft("Larger files are closer to the original and slower.").Pad(0, 6, 0, 10));
            Add(Ui.Line());
            foreach (var o in d.Variants)
            {
                var on = o.Variant.Label == Chosen?.Variant.Label;
                var right = Ui.Row(Ui.Fig(Show.Size(o.Variant.Size), 12, Theme.Faint).Mid(), new Border { Width = 112, Child = Rows.Speed(o.Prediction) }.Mid());
                Add(Ui.Choice(Ui.Ends(Ui.Fig(o.Variant.Label, 13, bold: on).Mid(), right), on, () => { label = o.Variant.Label; Build(); }, o.Variant.Label, new(12, 7, 6, 7)));
                Add(Ui.Line());
            }
        }
        if (d.Others.Count == 0) return;
        var list = new StackPanel { Margin = new(0, 8, 0, 0), Visibility = d.Variants.Count == 0 ? Visibility.Visible : Visibility.Collapsed };
        foreach (var f in d.Others)
        {
            var have = Store.Current.Find(d.Repo, [f]);
            UIElement get = have is null ? Ui.Outline("Get", () => Store.Current.Download(d.Id, [f])) : Ui.Soft(have.Ready ? "saved" : $"{(int)(have.Done / Math.Max(have.Total, 1) * 100)}%", 12);
            list.Children.Add(Ui.Ends(Ui.Fig(f.Path, 11.5).OneLine().Mid().Pad(0, 0, 10), Ui.Row(Ui.Fig(Show.Size(f.Size), 11.5, Theme.Faint).Mid().Pad(0, 0, 10), get)).Pad(0, 4));
        }
        var all = d.Variants.Count == 0 ? Ui.Outline($"Get all, {Show.Size(d.Others.Sum(f => f.Size))}", () => Store.Current.Download(d.Id, d.Others))
            : Ui.Outline("Show", () => list.Visibility = list.Visibility == Visibility.Visible ? Visibility.Collapsed : Visibility.Visible);
        Add(Ui.Ends(Ui.Head(d.Variants.Count == 0 ? "Files" : "Other files", 20).Mid(), all).Pad(0, 30));
        Add(list);
    }
}

public sealed class ItemProgress : StackPanel
{
    private readonly (Grid View, Action<double> Set) bar;
    private readonly TextBlock line;

    public ItemProgress(Item i)
    {
        var bad = i.Status == "failed";
        bar = Ui.Bar(8, bad ? Theme.Alarm : Theme.Ink, false);
        line = Ui.Fig("", 12, bad ? Theme.Alarm : Theme.Faint).Mid();
        var s = Store.Current;
        var buttons = Ui.Row(
            i.Status switch
            {
                "active" or "queued" => Ui.Outline("Pause", () => s.Act("pause", i.Key)),
                "paused" => Ui.Outline("Resume", () => s.Act("resume", i.Key)),
                _ => Ui.Outline("Try again", () => s.Act("resume", i.Key)),
            },
            Ui.Outline(bad ? "Remove" : "Cancel", () => s.Act("remove", i.Key)).Pad(8));
        Children.Add(bar.View);
        Children.Add(Ui.Ends(line.Pad(0, 0, 10), buttons).Pad(0, 8));
        Show(i);
    }

    public void Show(Item i)
    {
        bar.Set(i.Done / Math.Max(i.Total, 1));
        line.Text = i.Status switch
        {
            "queued" => "Waiting",
            "failed" => i.Error ?? "Failed",
            "paused" => $"Paused at {RunIt.Show.Size(i.Done)} of {RunIt.Show.Size(i.Total)}",
            _ => i.Done >= i.Total ? "Checking" : $"{RunIt.Show.Size(i.Done)} of {RunIt.Show.Size(i.Total)}  {RunIt.Show.Size(i.BytesPerS)}/s  {RunIt.Show.Left(i)}",
        };
    }
}
