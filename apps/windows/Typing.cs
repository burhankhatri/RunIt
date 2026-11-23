using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;

namespace RunIt;

/// Which characters of a text are written, part written, or still to come.
public readonly struct Reveal(double shown, int count)
{
    public const int Trail = 5;
    public int Solid { get; } = Math.Min(count, Math.Max(0, (int)shown - Trail));
    public int End { get; } = Math.Max(Math.Min(count, Math.Max(0, (int)shown - Trail)), Math.Min((int)shown, count));
    public bool Done => Solid >= count;
    public double Opacity(int i) => Math.Clamp((shown - i) / Trail, 0, 1);
}

/// Text that writes itself behind a marker caret. The whole text is laid out from the start, so nothing shifts.
public sealed class Typed : Grid
{
    private readonly TextBlock text;
    private readonly Rectangle caret = new() { Fill = Theme.Marker, HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Top, Visibility = Visibility.Collapsed };
    private readonly Stopwatch clock = new();
    private readonly double speed, delay;
    private readonly Action? done;
    private readonly TextEffect rest = new() { Foreground = Brushes.Transparent };
    private readonly TextEffect[] tail;
    private bool running;

    public Typed(TextBlock text, double perSecond = 46, double delay = 0, bool showCaret = true, Action? done = null)
    {
        (this.text, speed, this.delay, this.done) = (text, perSecond, delay, done);
        tail = [.. Enumerable.Range(0, Reveal.Trail).Select(_ => new TextEffect { PositionCount = 1 })];
        if (showCaret) Children.Add(caret);
        Children.Add(text);
        if (Theme.Still) { Loaded += (_, _) => done?.Invoke(); return; }
        text.TextEffects = [.. tail, rest];
        Draw(0);
        Loaded += (_, _) => Begin();
        Unloaded += (_, _) => End();
    }

    private void Begin() { if (running || text.TextEffects is null) return; running = true; clock.Restart(); CompositionTarget.Rendering += Tick; }
    private void End() { if (!running) return; running = false; CompositionTarget.Rendering -= Tick; }

    private void Tick(object? s, EventArgs e)
    {
        var shown = Math.Max(0, (clock.Elapsed.TotalSeconds - delay) * speed);
        if (new Reveal(shown, text.Text.Length).Done)
        {
            End();
            text.TextEffects = null;
            caret.Visibility = Visibility.Collapsed;
            done?.Invoke();
            return;
        }
        Draw(shown);
    }

    private void Draw(double shown)
    {
        var n = text.Text.Length;
        var r = new Reveal(shown, n);
        for (var k = 0; k < tail.Length; k++)
        {
            var i = r.Solid + k;
            tail[k].PositionStart = i;
            tail[k].PositionCount = i < r.End ? 1 : 0;
            if (i < r.End) tail[k].Foreground = new SolidColorBrush(((SolidColorBrush)text.Foreground).Color) { Opacity = r.Opacity(i) };
        }
        rest.PositionStart = r.End;
        rest.PositionCount = Math.Max(0, n - r.End);
        if (shown > 0 && r.End < n && text.ActualWidth > 0)
        {
            var box = text.ContentStart.GetPositionAtOffset(r.End + 1)?.GetCharacterRect(System.Windows.Documents.LogicalDirection.Forward) ?? Rect.Empty;
            if (!box.IsEmpty)
            {
                caret.Margin = new(box.X + 1, box.Y + box.Height * 0.14, 0, 0);
                (caret.Width, caret.Height) = (Math.Max(5, text.FontSize * 0.36), box.Height * 0.76);
                caret.Visibility = Visibility.Visible;
            }
        }
        else caret.Visibility = Visibility.Collapsed;
    }
}

/// A sample reply written at exactly the speed a model is predicted to write.
public sealed class SpeedPreview : Grid
{
    private readonly Stopwatch clock = Stopwatch.StartNew();
    private readonly TextBlock text;
    private readonly int[] ends;
    private readonly double speed;
    private readonly TextEffect rest = new() { Foreground = Brushes.Transparent };
    private int shown = -1;

    public SpeedPreview(double tokensPerSecond, bool code)
    {
        speed = Math.Max(tokensPerSecond, 0.1);
        var tokens = code ? Samples.Code : Samples.Prose;
        ends = new int[tokens.Count + 1];
        for (var i = 0; i < tokens.Count; i++) ends[i + 1] = ends[i] + tokens[i].Length;
        text = code ? Ui.Text(string.Concat(tokens), Theme.Mono, 12.5, line: 19) : Ui.Text(string.Concat(tokens), Theme.Reading, 16, line: 25);
        System.Windows.Automation.AutomationProperties.SetName(this, $"Sample text written at {Show.Speed(tokensPerSecond)} tokens a second");
        Children.Add(text);
        if (Theme.Still) return;
        text.TextEffects = [rest];
        Loaded += (_, _) => { CompositionTarget.Rendering -= Tick; CompositionTarget.Rendering += Tick; };
        Unloaded += (_, _) => CompositionTarget.Rendering -= Tick;
    }

    private void Tick(object? s, EventArgs e)
    {
        var length = (ends.Length - 1) / speed;
        var n = Math.Min(ends.Length - 1, (int)(clock.Elapsed.TotalSeconds % (length + 2.4) * speed));
        if (n == shown) return;
        shown = n;
        rest.PositionStart = ends[n];
        rest.PositionCount = ends[^1] - ends[n];
    }
}

public static class Samples
{
    /// Split the way tokenizers roughly do: words, with long ones in pieces.
    public static List<string> Split(string s)
    {
        List<string> list = [];
        var word = "";
        foreach (var c in s)
        {
            if (c is ' ' or '\n' || word.Count(x => x != ' ') >= 6) { if (word != "") list.Add(word); word = ""; }
            word += c;
        }
        if (word != "") list.Add(word);
        return list;
    }
    public static readonly List<string> Prose = Split("Tides are the ocean answering the moon. Its gravity pulls the near side of the Earth a little harder than the far side, and the water stretches into two bulges. The planet turns beneath them, so most coasts rise and fall twice a day. The sun joins in as well. When all three line up, the range is widest, and sailors call it a spring tide.");
    public static readonly List<string> Code = Split("def moving_average(values, window):\n    total = 0.0\n    out = []\n    for i, v in enumerate(values):\n        total += v\n        if i >= window:\n            total -= values[i - window]\n        if i >= window - 1:\n            out.append(total / window)\n    return out\n");
}

/// A figure that rolls up to its value.
public sealed class Rolling : TextBlock
{
    private static readonly DependencyProperty ShownProperty = DependencyProperty.Register("Shown", typeof(double), typeof(Rolling), new(0.0, (d, e) => ((Rolling)d).Text = Show.Speed((double)e.NewValue)));
    public Rolling(double value, double size)
    {
        (FontFamily, FontSize, Foreground, Text) = (Theme.Display, size, Theme.Ink, "0");
        (LineHeight, LineStackingStrategy) = (size, LineStackingStrategy.BlockLineHeight);
        if (Theme.Still) SetValue(ShownProperty, value);
        else BeginAnimation(ShownProperty, new DoubleAnimation(0, value, TimeSpan.FromSeconds(0.6)) { EasingFunction = Ui.Ease });
    }
}

/// Text a highlighter has been drawn across.
public sealed class Marked : Grid
{
    public Marked(string s, double size, double delay)
    {
        HorizontalAlignment = HorizontalAlignment.Left;
        var text = Ui.Text(s, Theme.Display, size, Theme.Dark ? Theme.Ink : Theme.OnMarker, italic: true);
        text.Margin = new(size * 0.12, 0, size * 0.16, 0);
        Margin = new(-size * 0.12, 0, 0, 0);
        var stroke = new Path { Fill = Theme.Marker, Stretch = Stretch.Fill, Margin = new(0, size * 0.12, 0, size * 0.06), Data = Geometry.Parse("M0,6 L100,0 L99.4,96 L0.4,100 Z"), RenderTransform = new ScaleTransform(Theme.Still ? 1 : 0, 1) };
        Children.Add(stroke);
        Children.Add(text);
        if (Theme.Still) { text.Foreground = Theme.OnMarker; return; }
        var at = TimeSpan.FromSeconds(delay);
        var sweep = new DoubleAnimation(0, 1, TimeSpan.FromSeconds(0.55)) { BeginTime = at, EasingFunction = new CubicEase { EasingMode = EasingMode.EaseInOut } };
        stroke.RenderTransform.BeginAnimation(ScaleTransform.ScaleXProperty, sweep);
        var ink = new SolidColorBrush(((SolidColorBrush)text.Foreground).Color);
        text.Foreground = ink;
        ink.BeginAnimation(SolidColorBrush.ColorProperty, new ColorAnimation(Theme.OnMarker.Color, TimeSpan.FromSeconds(0.4)) { BeginTime = at });
    }
}
