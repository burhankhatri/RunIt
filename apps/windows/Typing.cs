using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;

namespace RunIt;

/// How much of a text is written.
public readonly struct Reveal(double shown, int count)
{
    public int End { get; } = Math.Clamp((int)shown, 0, count);
    public bool Done => shown >= count;
}

/// Text that writes itself behind a marker caret. The whole text is laid out from the start, so nothing shifts.
public sealed class Typed : Grid
{
    private readonly TextBlock text;
    private readonly Rectangle caret = new() { Fill = Theme.Marker, HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Top, Visibility = Visibility.Collapsed };
    private readonly Stopwatch clock = new();
    private readonly double speed, delay;
    private readonly Action? done;
    private readonly string words;
    private readonly Run written = new(), ahead = new() { Foreground = Brushes.Transparent };
    private bool running, finished;
    private int last = -1;

    public Typed(TextBlock text, double perSecond = 46, double delay = 0, bool showCaret = true, Action? done = null)
    {
        (this.text, speed, this.delay, this.done, words) = (text, perSecond, delay, done, text.Text);
        if (showCaret) Children.Add(caret);
        Children.Add(text);
        System.Windows.Automation.AutomationProperties.SetName(this, words);
        if (Theme.Still) { Loaded += (_, _) => done?.Invoke(); return; }
        text.Inlines.Clear();
        text.Inlines.Add(written);
        text.Inlines.Add(ahead);
        Draw(0);
        Loaded += (_, _) => Begin();
        Unloaded += (_, _) => End();
    }

    private void Begin() { if (running || finished) return; running = true; clock.Restart(); CompositionTarget.Rendering += Tick; }
    private void End() { if (!running) return; running = false; CompositionTarget.Rendering -= Tick; }

    private void Tick(object? s, EventArgs e)
    {
        var shown = Math.Max(0, (clock.Elapsed.TotalSeconds - delay) * speed);
        if (new Reveal(shown, words.Length).Done)
        {
            End();
            finished = true;
            text.Text = words;
            caret.Visibility = Visibility.Collapsed;
            done?.Invoke();
            return;
        }
        Draw(shown);
    }

    private void Draw(double shown)
    {
        var end = new Reveal(shown, words.Length).End;
        if (end == last) return;
        last = end;
        // Both runs are always present, so the words never move. (Text effects would do, but a
        // TextBlock stops applying them once anything asks it where a character is.)
        written.Text = words[..end];
        ahead.Text = words[end..];
        if (shown <= 0 || end >= words.Length || !text.IsMeasureValid && text.ActualWidth == 0) { caret.Visibility = Visibility.Collapsed; return; }
        text.UpdateLayout();
        var box = ahead.ContentStart.GetCharacterRect(System.Windows.Documents.LogicalDirection.Forward);
        if (box.IsEmpty) return;
        caret.Margin = new(box.X + 1, box.Y + box.Height * 0.14, 0, 0);
        (caret.Width, caret.Height) = (Math.Max(5, text.FontSize * 0.36), box.Height * 0.76);
        caret.Visibility = Visibility.Visible;
    }
}

/// A sample reply written at exactly the speed a model is predicted to write.
public sealed class SpeedPreview : Grid
{
    private readonly Stopwatch clock = Stopwatch.StartNew();
    private readonly TextBlock text;
    private readonly int[] ends;
    private readonly double speed;
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
        text.TextEffects = [new() { PositionStart = 0, PositionCount = ends[^1], Foreground = Brushes.Transparent }];
        Loaded += (_, _) => { CompositionTarget.Rendering -= Tick; CompositionTarget.Rendering += Tick; };
        Unloaded += (_, _) => CompositionTarget.Rendering -= Tick;
    }

    private void Tick(object? s, EventArgs e)
    {
        var length = (ends.Length - 1) / speed;
        var n = Math.Min(ends.Length - 1, (int)(clock.Elapsed.TotalSeconds % (length + 2.4) * speed));
        if (n == shown) return;
        shown = n;
        text.TextEffects = ends[n] < ends[^1] ? [new() { PositionStart = ends[n], PositionCount = ends[^1] - ends[n], Foreground = Brushes.Transparent }] : null;
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
        var stroke = new Path { Fill = Theme.Marker, RenderTransform = new ScaleTransform(Theme.Still ? 1 : 0, 1) };
        // On a canvas, so the stroke follows the words and never sizes them.
        Children.Add(new Canvas { Children = { stroke } });
        SizeChanged += (_, e) =>
        {
            var (w, h) = (e.NewSize.Width, e.NewSize.Height);
            stroke.Data = Geometry.Parse(FormattableString.Invariant($"M0,{h * 0.15} L{w},{h * 0.10} L{w - 3},{h * 0.94} L2,{h * 0.98} Z"));
        };
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
