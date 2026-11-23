using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;
using Microsoft.Win32;

namespace RunIt;

public static class Theme
{
    public static bool Dark { get; private set; }
    public static SolidColorBrush Paper = null!, Sheet = null!, Ink = null!, Faint = null!, Rule = null!, Marker = null!, OnMarker = null!, Alarm = null!, Wash = null!;

    private static readonly Uri Root = new("pack://application:,,,/");
    public static readonly FontFamily Display = new(Root, "./fonts/#RunIt Display"), Reading = new(Root, "./fonts/#RunIt Text"), Sans = new(Root, "./fonts/#RunIt Sans"), Mono = new(Root, "./fonts/#RunIt Mono");

    /// Off when the person asked Windows for no animation.
    public static bool Still => !SystemParameters.ClientAreaAnimation && Rehearsal.Folder is null;

    private static SolidColorBrush B(uint light, uint dark)
    {
        var h = Dark ? dark : light;
        var b = new SolidColorBrush(Color.FromRgb((byte)(h >> 16), (byte)(h >> 8), (byte)h));
        b.Freeze();
        return b;
    }

    public static void Apply()
    {
        Dark = Environment.GetEnvironmentVariable("RUNIT_LOOK") switch
        {
            "dark" => true,
            "light" => false,
            _ => Registry.GetValue(@"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "AppsUseLightTheme", 1) is 0,
        };
        Paper = B(0xE8ECE1, 0x0C1511); Sheet = B(0xF3F6EE, 0x131F19); Ink = B(0x0F2519, 0xE7EEE4); Faint = B(0x56685C, 0x93A599);
        Rule = B(0xCBD3C5, 0x25362C); Marker = B(0xF1FB5F, 0xD9EE4E); OnMarker = B(0x0F2519, 0x0F2519); Alarm = B(0xA8321A, 0xF08A6B); Wash = B(0xDDE3D6, 0x1B2A22);
        var r = Application.Current.Resources;
        (r["Paper"], r["Ink"], r["Rule"], r["Marker"], r["Wash"]) = (Paper, Ink, Rule, Marker, Wash);
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);

    /// Colour the title bar to match, where Windows supports it.
    public static void TitleBar(Window w)
    {
        var h = new WindowInteropHelper(w).Handle;
        int dark = Dark ? 1 : 0, c = Paper.Color.R | Paper.Color.G << 8 | Paper.Color.B << 16, t = Ink.Color.R | Ink.Color.G << 8 | Ink.Color.B << 16;
        _ = DwmSetWindowAttribute(h, 20, ref dark, 4);
        _ = DwmSetWindowAttribute(h, 35, ref c, 4);
        _ = DwmSetWindowAttribute(h, 36, ref t, 4);
    }
}

/// Short ways to build what the screens are made of.
public static class Ui
{
    public static TextBlock Text(string s, FontFamily f, double size, Brush? color = null, bool bold = false, bool italic = false, double line = 0) => new()
    {
        Text = s, FontFamily = f, FontSize = size, Foreground = color ?? Theme.Ink, TextWrapping = TextWrapping.Wrap,
        FontWeight = bold ? FontWeights.Bold : FontWeights.Normal, FontStyle = italic ? FontStyles.Italic : FontStyles.Normal,
        LineHeight = line > 0 ? line : double.NaN, LineStackingStrategy = line > 0 ? LineStackingStrategy.BlockLineHeight : LineStackingStrategy.MaxHeight,
    };
    public static TextBlock Head(string s, double size, Brush? c = null) => Text(s, Theme.Display, size, c, line: size * 1.08);
    public static TextBlock Body(string s, double size = 13.5, Brush? c = null, bool bold = false) => Text(s, Theme.Sans, size, c, bold);
    public static TextBlock Soft(string s, double size = 12.5) => Text(s, Theme.Sans, size, Theme.Faint);
    public static TextBlock Fig(string s, double size = 13, Brush? c = null, bool bold = false) => Text(s, Theme.Mono, size, c, bold);

    public static T Pad<T>(this T e, double l = 0, double t = 0, double r = 0, double b = 0) where T : FrameworkElement { e.Margin = new(l, t, r, b); return e; }
    public static T Max<T>(this T e, double w) where T : FrameworkElement { e.MaxWidth = w; e.HorizontalAlignment = HorizontalAlignment.Left; return e; }
    public static T Left<T>(this T e) where T : FrameworkElement { e.HorizontalAlignment = HorizontalAlignment.Left; return e; }
    public static T Right<T>(this T e) where T : FrameworkElement { e.HorizontalAlignment = HorizontalAlignment.Right; return e; }
    public static T Mid<T>(this T e) where T : FrameworkElement { e.VerticalAlignment = VerticalAlignment.Center; return e; }
    public static TextBlock OneLine(this TextBlock t) { t.TextWrapping = TextWrapping.NoWrap; t.TextTrimming = TextTrimming.CharacterEllipsis; return t; }

    public static StackPanel Stack(params UIElement?[] c) { var p = new StackPanel(); foreach (var x in c) if (x is not null) p.Children.Add(x); return p; }
    public static StackPanel Row(params UIElement?[] c) { var p = Stack(c); p.Orientation = Orientation.Horizontal; return p; }
    /// Something at each end.
    public static Grid Ends(UIElement a, UIElement b)
    {
        var g = new Grid();
        g.ColumnDefinitions.Add(new() { Width = new(1, GridUnitType.Star) });
        g.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        Grid.SetColumn(b, 1);
        g.Children.Add(a); g.Children.Add(b);
        return g;
    }
    public static Rectangle Line() => new() { Height = 1, Fill = Theme.Rule, SnapsToDevicePixels = true };
    public static Rectangle Tall() => new() { Width = 1, Fill = Theme.Rule, SnapsToDevicePixels = true };

    public static Button Solid(string s, Action act) => Make("Solid", Text(s, Theme.Sans, 14, Theme.Paper, true), act);
    public static Button Outline(string s, Action act) => Make("Outline", Text(s, Theme.Sans, 13), act);
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
    public static ScrollViewer Scroll(FrameworkElement c) => new() { Content = c, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, Focusable = false };

    /// A row that can be chosen: hover wash, marker edge when on.
    public static Button Choice(UIElement content, bool on, Action act, string name, Thickness pad)
    {
        var edge = new Rectangle { Width = 4, Fill = Theme.Marker, HorizontalAlignment = HorizontalAlignment.Left, Visibility = on ? Visibility.Visible : Visibility.Hidden };
        var back = new Border { Background = on ? Theme.Sheet : Brushes.Transparent, Child = new Border { Padding = pad, Child = content } };
        var g = new Grid { Children = { back, edge } };
        if (!on) { g.MouseEnter += (_, _) => back.Background = Theme.Wash; g.MouseLeave += (_, _) => back.Background = Brushes.Transparent; }
        return Bare(g, act, name);
    }

    public static readonly IEasingFunction Ease = new QuinticEase { EasingMode = EasingMode.EaseOut };
    /// Fade and lift into place.
    public static T Rise<T>(this T e, double delay = 0) where T : FrameworkElement
    {
        if (Theme.Still) return e;
        var shift = new TranslateTransform(0, 10);
        e.RenderTransform = shift;
        e.Opacity = 0;
        e.BeginAnimation(UIElement.OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromSeconds(0.42)) { BeginTime = TimeSpan.FromSeconds(delay), EasingFunction = Ease });
        shift.BeginAnimation(TranslateTransform.YProperty, new DoubleAnimation(10, 0, TimeSpan.FromSeconds(0.5)) { BeginTime = TimeSpan.FromSeconds(delay), EasingFunction = Ease });
        return e;
    }

    /// A bar that fills to a share of its width.
    public static (Grid View, Action<double> Set) Bar(double height, Brush fill, bool glide = true)
    {
        var scale = new ScaleTransform(0, 1);
        var top = new Rectangle { Fill = fill, RadiusX = 3, RadiusY = 3, RenderTransform = scale };
        var g = new Grid { Height = height, Children = { new Rectangle { Fill = Theme.Rule, RadiusX = 3, RadiusY = 3 }, top }, ClipToBounds = true };
        return (g, v =>
        {
            v = Math.Clamp(double.IsFinite(v) ? v : 0, 0, 1);
            if (glide && !Theme.Still) scale.BeginAnimation(ScaleTransform.ScaleXProperty, new DoubleAnimation(v, TimeSpan.FromSeconds(0.7)) { EasingFunction = Ease });
            else { scale.BeginAnimation(ScaleTransform.ScaleXProperty, null); scale.ScaleX = v; }
        });
    }

    /// Listen to the store for as long as an element is on screen.
    public static T Listen<T>(this T e, Action<string> on) where T : FrameworkElement
    {
        e.Loaded += (_, _) => { Store.Current.Changed -= on; Store.Current.Changed += on; };
        e.Unloaded += (_, _) => Store.Current.Changed -= on;
        return e;
    }
}
