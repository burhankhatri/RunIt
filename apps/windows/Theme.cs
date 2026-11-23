using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;

namespace RunIt;

public static class Theme
{
    private static readonly Uri Root = new("pack://application:,,,/");
    public static readonly FontFamily Sans = new(Root, "./fonts/#RunIt Sans"), Mono = new(Root, "./fonts/#RunIt Mono"), Pixel = new(Root, "./fonts/#RunIt Pixel");

    public static readonly SolidColorBrush Ground = B(0x1B1B1D), Card = B(0x262629), Raised = B(0x2E2E32), Line = B(0x303034), Off = B(0x37373B), Text = B(0xF4F4F2), Soft = B(0xA6A6AA),
        Ink = B(0x111111), InkSoft = B(0x111111, 0.72), Orange = B(0xFF7A00), Alarm = B(0xFF6B52);
    /// Flat blocks. Ink reads on all of them.
    public static readonly SolidColorBrush[] Tints = [Orange, B(0xFFD400), B(0xF4F4F2), B(0xFF5A3C), B(0x62C8FF)];

    /// Off when the person asked Windows for no animation.
    public static bool Still => !SystemParameters.ClientAreaAnimation && Rehearsal.Folder is null;

    /// Another app is in front, so nothing needs to move.
    public static bool Resting => Rehearsal.Folder is null && Application.Current.MainWindow is { IsActive: false };

    private static SolidColorBrush B(uint h, double opacity = 1)
    {
        var b = new SolidColorBrush(Color.FromRgb((byte)(h >> 16), (byte)(h >> 8), (byte)h)) { Opacity = opacity };
        b.Freeze();
        return b;
    }

    public static void Apply()
    {
        var r = Application.Current.Resources;
        (r["Ground"], r["Raised"], r["Off"], r["Text"], r["Orange"]) = (Ground, Raised, Off, Text, Orange);
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);

    /// Colour the title bar to match, where Windows supports it.
    public static void TitleBar(Window w)
    {
        var h = new WindowInteropHelper(w).Handle;
        int dark = 1, c = Ground.Color.R | Ground.Color.G << 8 | Ground.Color.B << 16, t = Soft.Color.R | Soft.Color.G << 8 | Soft.Color.B << 16;
        _ = DwmSetWindowAttribute(h, 20, ref dark, 4);
        _ = DwmSetWindowAttribute(h, 35, ref c, 4);
        _ = DwmSetWindowAttribute(h, 36, ref t, 4);
    }
}

/// Short ways to build what the screens are made of.
public static class Ui
{
    public static TextBlock Text(string s, FontFamily f, double size, Brush? color = null, bool bold = false, double line = 0) => new()
    {
        Text = s, FontFamily = f, FontSize = size, Foreground = color ?? Theme.Text, TextWrapping = TextWrapping.Wrap, FontWeight = bold ? FontWeights.Bold : FontWeights.Normal,
        LineHeight = line > 0 ? line : double.NaN, LineStackingStrategy = line > 0 ? LineStackingStrategy.BlockLineHeight : LineStackingStrategy.MaxHeight,
    };
    public static TextBlock Bold(string s, double size = 14, Brush? c = null) => Text(s, Theme.Sans, size, c, true);
    public static TextBlock Body(string s, double size = 13.5, Brush? c = null) => Text(s, Theme.Sans, size, c);
    public static TextBlock Soft(string s, double size = 12.5) => Text(s, Theme.Sans, size, Theme.Soft);
    public static TextBlock Pixel(string s, double size, Brush? c = null) => Text(s, Theme.Pixel, size, c).OneLine();

    public static T Pad<T>(this T e, double l = 0, double t = 0, double r = 0, double b = 0) where T : FrameworkElement { e.Margin = new(l, t, r, b); return e; }
    public static T Max<T>(this T e, double w) where T : FrameworkElement { e.MaxWidth = w; e.HorizontalAlignment = HorizontalAlignment.Left; return e; }
    public static T Wide<T>(this T e, double w) where T : FrameworkElement { e.Width = w; return e; }
    public static T Left<T>(this T e) where T : FrameworkElement { e.HorizontalAlignment = HorizontalAlignment.Left; return e; }
    public static T Right<T>(this T e) where T : FrameworkElement { e.HorizontalAlignment = HorizontalAlignment.Right; return e; }
    public static T Centre<T>(this T e) where T : FrameworkElement { e.HorizontalAlignment = HorizontalAlignment.Center; return e; }
    public static T Mid<T>(this T e) where T : FrameworkElement { e.VerticalAlignment = VerticalAlignment.Center; return e; }
    public static T Top<T>(this T e) where T : FrameworkElement { e.VerticalAlignment = VerticalAlignment.Top; return e; }
    public static T Foot<T>(this T e) where T : FrameworkElement { e.VerticalAlignment = VerticalAlignment.Bottom; return e; }
    public static TextBlock OneLine(this TextBlock t) { t.TextWrapping = TextWrapping.NoWrap; t.TextTrimming = TextTrimming.CharacterEllipsis; return t; }

    public static StackPanel Stack(params UIElement?[] c) { var p = new StackPanel(); foreach (var x in c) if (x is not null) p.Children.Add(x); return p; }
    public static StackPanel Row(params UIElement?[] c) { var p = Stack(c); p.Orientation = Orientation.Horizontal; return p; }
    /// Something at each end.
    public static Grid Ends(UIElement a, UIElement b) => Columns([(a, 0), (b, -1)]);
    /// Widths: 0 fills, -1 fits, anything else is a share.
    public static Grid Columns((UIElement View, double Width)[] cells, double gap = 0)
    {
        var g = new Grid();
        for (var n = 0; n < cells.Length; n++)
        {
            if (n > 0 && gap > 0) g.ColumnDefinitions.Add(new() { Width = new(gap) });
            var w = cells[n].Width;
            g.ColumnDefinitions.Add(new() { Width = w < 0 ? GridLength.Auto : new(w == 0 ? 1 : w, GridUnitType.Star) });
            Grid.SetColumn(cells[n].View, g.ColumnDefinitions.Count - 1);
            g.Children.Add(cells[n].View);
        }
        return g;
    }
    public static Rectangle Line() => new() { Height = 1, Fill = Theme.Line, SnapsToDevicePixels = true };
    public static Border Box(UIElement c, Brush? fill = null, double radius = 14, double pad = 20) => new() { Background = fill ?? Theme.Card, CornerRadius = new(radius), Padding = new(pad), Child = c };

    public static Button Block(string s, Action act, Brush? fill = null, Brush? label = null, bool tall = false)
    {
        var b = Make("Block", Text(s, Theme.Sans, tall ? 15 : 13, label ?? Theme.Ink, true).OneLine(), act);
        if (fill is not null) b.Background = fill;
        if (tall) (b.Height, b.Padding) = (44, new(20, 0, 20, 0));
        return b;
    }
    public static Button Plain(string s, Action act) => Make("Plain", Text(s, Theme.Sans, 13).OneLine(), act);
    public static Button Bare(UIElement content, Action act, string? name = null)
    {
        var b = Make("Bare", content, act);
        if (name is not null) System.Windows.Automation.AutomationProperties.SetName(b, name);
        return b;
    }
    private static Button Make(string style, UIElement content, Action act)
    {
        var b = new Button { Content = content, Style = (Style)Application.Current.Resources[style] };
        b.Click += (_, _) => act();
        return b;
    }
    /// A scrolling page: one centred column.
    public static ScrollViewer Page(FrameworkElement c, double max = 1120)
    {
        var g = new Grid { Margin = new(36, 30, 36, 36) };
        g.ColumnDefinitions.Add(new() { Width = new(0.0001, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new() { Width = new(1, GridUnitType.Star), MaxWidth = max });
        g.ColumnDefinitions.Add(new() { Width = new(0.0001, GridUnitType.Star) });
        Grid.SetColumn(c, 1);
        g.Children.Add(c);
        return Scroll(g);
    }
    public static ScrollViewer Scroll(FrameworkElement c) => new() { Content = c, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, Focusable = false };

    /// Hop up a little while the pointer is over `host`.
    public static void Hop(FrameworkElement host, FrameworkElement jumper, double by)
    {
        var shift = new TranslateTransform();
        jumper.RenderTransform = shift;
        void To(double y)
        {
            if (Theme.Still) return;
            shift.BeginAnimation(TranslateTransform.YProperty, new DoubleAnimation(y, TimeSpan.FromSeconds(0.28)) { EasingFunction = new BackEase { EasingMode = EasingMode.EaseOut, Amplitude = 0.9 } });
        }
        host.MouseEnter += (_, _) => To(-by);
        host.MouseLeave += (_, _) => To(0);
    }

    public static readonly IEasingFunction Ease = new QuinticEase { EasingMode = EasingMode.EaseOut };
    /// Fade and lift into place.
    public static T Rise<T>(this T e, double delay = 0, double from = 22) where T : FrameworkElement
    {
        if (Theme.Still) return e;
        var shift = new TranslateTransform(0, from);
        e.RenderTransform = shift;
        e.Opacity = 0;
        var at = TimeSpan.FromSeconds(delay);
        e.BeginAnimation(UIElement.OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromSeconds(0.3)) { BeginTime = at, EasingFunction = Ease });
        shift.BeginAnimation(TranslateTransform.YProperty, new DoubleAnimation(from, 0, TimeSpan.FromSeconds(0.45)) { BeginTime = at, EasingFunction = new BackEase { EasingMode = EasingMode.EaseOut, Amplitude = 0.35 } });
        return e;
    }

    /// Listen to the store for as long as an element is on screen.
    public static T Listen<T>(this T e, Action<string> on) where T : FrameworkElement
    {
        e.Loaded += (_, _) => { Store.Current.Changed -= on; Store.Current.Changed += on; };
        e.Unloaded += (_, _) => Store.Current.Changed -= on;
        return e;
    }
}
