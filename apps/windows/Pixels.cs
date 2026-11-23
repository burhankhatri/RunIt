using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;

namespace RunIt;

/// A model's creature, walking on the spot. Square cells, no soft edges.
public sealed class Creature : FrameworkElement
{
    private readonly string name;
    private readonly Brush color;
    private readonly DispatcherTimer timer;
    private int frame;

    public Creature(string name, double side, Brush? color = null, double pace = 0.46)
    {
        (this.name, this.color, Width, Height) = (name, color ?? Theme.Ink, side, side);
        IsHitTestVisible = false;
        RenderOptions.SetEdgeMode(this, EdgeMode.Aliased);
        timer = new(DispatcherPriority.Render) { Interval = TimeSpan.FromSeconds(pace) };
        timer.Tick += (_, _) => { if (Theme.Resting) return; frame ^= 1; InvalidateVisual(); };
        Loaded += (_, _) => { if (!Theme.Still) timer.Start(); };
        Unloaded += (_, _) => timer.Stop();
    }

    public double Pace { set => timer.Interval = TimeSpan.FromSeconds(value); }

    protected override void OnRender(DrawingContext d)
    {
        var frames = Store.Current.Creature(name);
        if (frames.Length != 2) return;
        var c = Width / 10;
        for (var y = 0; y < 10; y++)
            for (var x = 0; x < 10; x++)
                if ((frames[frame][y] >> x & 1) == 1) d.DrawRectangle(color, null, new(x * c, y * c, c, c));
    }
}

/// This computer's creature, put together one block at a time as the test runs.
public sealed class Building : FrameworkElement
{
    private readonly string name;
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromSeconds(0.2) };
    private bool blink;

    public Building(string name, double side)
    {
        (this.name, Width, Height) = (name, side, side);
        RenderOptions.SetEdgeMode(this, EdgeMode.Aliased);
        timer.Tick += (_, _) => { blink = !blink; InvalidateVisual(); };
        Loaded += (_, _) => timer.Start();
        Unloaded += (_, _) => timer.Stop();
    }

    protected override void OnRender(DrawingContext d)
    {
        var rows = Store.Current.Creature(name).FirstOrDefault() ?? [];
        var c = Width / 10;
        for (var y = 0; y < 10; y++) for (var x = 0; x < 10; x++) d.DrawRectangle(Theme.Card, null, new(x * c + 1, y * c + 1, c - 2, c - 2));
        if (rows.Length != 10) return;
        // Scattered, but the same every time.
        var order = Enumerable.Range(0, 100).Where(i => (rows[i / 10] >> i % 10 & 1) == 1).OrderBy(i => (i % 10 * 37 + i / 10 * 59) % 101).ToList();
        var progress = Store.Current.Progress;
        if (progress <= 0) return;
        var n = (int)(progress * order.Count);
        for (var k = 0; k <= n && k < order.Count; k++)
        {
            // The block being laid blinks; the rest are solid.
            if (k == n && progress < 1 && blink) continue;
            d.DrawRectangle(Theme.Orange, null, new(order[k] % 10 * c, order[k] / 10 * c, c, c));
        }
    }
}

/// A row of blocks that light up one after another.
public sealed class Blocks : FrameworkElement
{
    private static readonly DependencyProperty LitProperty = DependencyProperty.Register("Lit", typeof(double), typeof(Blocks), new FrameworkPropertyMetadata(0.0, FrameworkPropertyMetadataOptions.AffectsRender));
    private readonly int count;
    private readonly double cell;
    private readonly Brush color;

    public Blocks(double share, int count = 24, double cell = 9, double height = 14, Brush? color = null)
    {
        (this.count, this.cell, this.color) = (count, cell, color ?? Theme.Orange);
        (Width, Height) = (count * (cell + 3) - 3, height);
        HorizontalAlignment = HorizontalAlignment.Left;
        VerticalAlignment = VerticalAlignment.Center;
        RenderOptions.SetEdgeMode(this, EdgeMode.Aliased);
        Set(share);
    }

    public void Set(double share)
    {
        var want = !double.IsFinite(share) || share <= 0 ? 0.0 : Math.Clamp(Math.Round(share * count), 1, count);
        if (Theme.Still) SetValue(LitProperty, want);
        else BeginAnimation(LitProperty, new DoubleAnimation(want, TimeSpan.FromSeconds(0.1 + 0.016 * count)));
    }

    protected override void OnRender(DrawingContext d)
    {
        var lit = (double)GetValue(LitProperty);
        for (var i = 0; i < count; i++) d.DrawRectangle(i < Math.Floor(lit + 0.001) ? color : Theme.Off, null, new(i * (cell + 3), 0, cell, Height));
    }
}

/// A figure that counts up.
public sealed class Figure : TextBlock
{
    private static readonly DependencyProperty ShownProperty = DependencyProperty.Register("Shown", typeof(double), typeof(Figure), new(0.0, (d, e) => ((Figure)d).Text = Show.Speed((double)e.NewValue)));
    public Figure(double value, double size, Brush? color = null)
    {
        (FontFamily, FontSize, Foreground, Text) = (Theme.Pixel, size, color ?? Theme.Ink, "0.0");
        (LineHeight, LineStackingStrategy) = (size * 0.86, LineStackingStrategy.BlockLineHeight);
        System.Windows.Automation.AutomationProperties.SetName(this, Show.Speed(value));
        if (Theme.Still) SetValue(ShownProperty, value);
        else BeginAnimation(ShownProperty, new DoubleAnimation(0, value, TimeSpan.FromSeconds(0.55)) { EasingFunction = Ui.Ease });
    }
}

public static class Tile
{
    /// A creature on its colour.
    public static Border Of(string name, double side) => new()
    {
        Width = side, Height = side, CornerRadius = new(side * 0.2), Background = Store.Current.Tint(name),
        Child = new Creature(name, Math.Round(side * 0.066) * 10).Centre().Mid(),
    };
}
