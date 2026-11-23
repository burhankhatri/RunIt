using System.Globalization;
using System.Text.Json.Serialization;

namespace RunIt;

public record Prediction(string Tier, double NeedBytes, double CapacityBytes, double TokensPerS, double TokensPerSLow, double TokensPerSHigh, double TokensPerSDeep,
    double TokensPerSSustained, double PromptTokensPerS, double GpuShare, bool Rough);
public record Rating(string Name, [property: JsonPropertyName("rating")] int Score, int Votes, int? Coding);
public record RemoteFile(string Path, double Size, string Sha256);
public record Variant(string Label, double Bits, double Size, List<RemoteFile> Files);
public record Choice(string Id, string Repo, string Name, string License, long Downloads, Rating? Rating, Variant Variant, Prediction Prediction);
public record Pick(string Role, string Title, string Why, List<string> Also, Choice Choice);
public record Shortlist(List<Pick> Picks, List<Choice> Ranked, List<Choice> Unrated, string RatingsDate, string CatalogDate, long CatalogTotal);
public record Hit(string Id, string Repo, string Name, string Kind, string License, long Downloads, List<string> Formats, double? Size, string? Label, Rating? Rating, Prediction? Prediction);
public record Page(List<Hit> Hits, long Total, string? Next);
public record Offer(Variant Variant, Prediction? Prediction);
public record Detail(string Id, string Repo, string Name, string License, string? Note, Rating? Rating, List<Offer> Variants, List<RemoteFile> Others, double? Capacity);
public record Device(string Id, string Name, double TotalMib, double FreeMib, bool Integrated);
public record Machine(string Os, string Cpu, int Cores, int Threads, int PerformanceCores, double RamBytes, double RamFreeBytes, Device? Gpu);
public record BenchRun(string Place, double ModelBytes, double Decode, double Prompt);
public record Calibration(double Sustained);
public record Report(Machine Machine, Calibration Calibration, List<BenchRun> Runs, double TakenAt, double Seconds, double BytesPerS);
public record Conditions(bool Hot, bool OnBattery);
public record Measured(double TokensPerS, double PromptTokensPerS, Conditions Conditions);
public record Item(string Key, string Id, string Repo, string Name, string Label, List<RemoteFile> Files, double Total, double Done, string Status, string? Error, Prediction? Prediction, Measured? Measured, double BytesPerS)
{
    [JsonIgnore] public bool Ready => Status == "ready";
    [JsonIgnore] public bool Runnable => Ready && Files.Count > 0 && Files[0].Path.EndsWith(".gguf", StringComparison.OrdinalIgnoreCase);
}
public record AppState(string Version, Report? Report, bool Engine, List<Item> Items, string ModelsDir, string RatingsDate);
public record Event(string Type, string? Kind, string? Key, string? Text, string? Error, string? Removed, double? Progress, double? TokensPerS, Report? Report, Item? Item, Measured? Measured);

public static class Show
{
    private static string F(FormattableString s) => s.ToString(CultureInfo.InvariantCulture);
    public static string Size(double b) => b >= 1e9 ? F($"{b / 1e9:0.0} GB") : b >= 1e6 ? F($"{b / 1e6:0} MB") : F($"{Math.Max(1, b / 1e3):0} KB");
    public static string Memory(double b) => F($"{b / 1073741824:0.0} GB");
    public static string Speed(double s) => s >= 10 ? F($"{s:0}") : F($"{s:0.0}");
    public static string Count(long n) => n >= 1_000_000 ? F($"{n / 1e6:0.0}M") : n >= 1000 ? $"{n / 1000}K" : $"{n}";
    public static string License(string l) => l is "" or "unknown" or "other" ? "licence not stated" : l;
    public static string Left(Item i)
    {
        if (i.BytesPerS <= 1000) return "";
        var s = (i.Total - i.Done) / i.BytesPerS;
        return s < 90 ? $"{(int)s} s left" : s < 5400 ? $"{(int)(s / 60)} min left" : F($"{s / 3600:0.0} h left");
    }
}
