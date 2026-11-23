using System.Windows;

namespace RunIt;

public enum Place { ForYou, Browse, Library, Machine }

/// A model opened from a list.
public sealed record Opened(string Id, string Name, Choice? Seed = null, bool Code = false);

public sealed class Turn(bool mine, string text) { public bool Mine { get; } = mine; public string Text { get; set; } = text; public string Thought { get; set; } = ""; }

/// Everything the screens show. Changes are announced by topic, on the UI thread.
public sealed class Store
{
    public static readonly Store Current = new();
    public event Action<string>? Changed;
    public void Say(string topic) => Changed?.Invoke(topic);

    public Core? Core { get; private set; }
    public string? Fatal { get; private set; }
    public bool Ready { get; private set; }
    public string? Notice { get; private set; }

    public Report? Report { get; private set; }
    public List<Item> Items { get; private set; } = [];
    public string ModelsDir { get; private set; } = "";
    public string Version { get; private set; } = "";
    public string RatingsDate { get; private set; } = "";

    public bool Testing { get; private set; }
    public List<string> Lines { get; } = [];
    public string Stage { get; private set; } = "";
    public double Progress { get; private set; }
    public string? TestError { get; private set; }
    public bool Entered { get; private set; }

    public Place Place { get; private set; }
    public Opened? Opened { get; private set; }
    public Shortlist? Shortlist { get; private set; }
    public double? CatalogProgress { get; private set; }
    public string? ShortlistError { get; private set; }

    public string Query { get; private set; } = "";
    public List<Hit> Hits { get; private set; } = [];
    public long Total { get; private set; }
    public string? Next { get; private set; }
    public bool Searching { get; private set; }
    public string? SearchError { get; private set; }
    private int searchTurn;

    public string? ChatKey { get; private set; }
    public bool ChatLoading { get; private set; }
    public bool ChatBusy { get; private set; }
    public string? ChatError { get; private set; }
    public string? ChatNote { get; private set; }
    public List<Turn> Turns { get; } = [];
    public double LiveSpeed { get; private set; }
    public HashSet<string> Measuring { get; } = [];

    public async void Start()
    {
        try { Core = new Core(e => Application.Current?.Dispatcher.BeginInvoke(() => Handle(e))); }
        catch (Exception e) when (e is CoreException or DllNotFoundException or BadImageFormatException)
        {
            Fatal = e is CoreException ? e.Message : "Part of RunIt is missing. Run the install command again.";
            Say("shell");
            return;
        }
        if (await Try(() => Core.Call<AppState>("state")) is not { } s) return;
        (Report, Items, ModelsDir, Version, RatingsDate) = (s.Report, s.Items, s.ModelsDir, s.Version, s.RatingsDate);
        if (!s.Engine) Fatal = "The llama.cpp engine is missing from this copy of RunIt. Run the install command again.";
        Entered = Report is not null;
        Ready = true;
        Say("shell");
        if (Report is not null) await Recommend();
    }

    public void Notify(string? text) { Notice = text; Say("notice"); }

    private async Task<T?> Try<T>(Func<Task<T>> f) where T : class
    {
        try { return await f(); } catch (Exception e) { Notify(e.Message); return null; }
    }
    private async Task<bool> Do(string method, object? p = null)
    {
        try { await Core!.Run(method, p); return true; } catch (Exception e) { Notify(e.Message); return false; }
    }

    // The test

    public async void Test()
    {
        Lines.Clear();
        (Progress, TestError, Testing, Entered) = (0, null, true, false);
        Say("shell");
        try { await Core!.Run("bench.start"); } catch (Exception e) { (Testing, TestError) = (false, e.Message); Say("shell"); }
    }
    public async void StopTest() => await Do("bench.stop");
    public void Enter() { Entered = true; Say("shell"); }

    public async Task Recommend()
    {
        ShortlistError = null;
        Say("shortlist");
        try { var s = await Core!.Call<Shortlist>("recommend"); AssignTints(s); Shortlist = s; } catch (Exception e) { ShortlistError = e.Message; }
        CatalogProgress = null;
        Say("shortlist");
    }

    public void Go(Place p)
    {
        if (ChatKey is not null) CloseChat();
        (Opened, Place) = (null, p);
        Say("place");
    }
    public void Open(Opened? o) { Opened = o; Say("place"); }

    // Looks

    private readonly Dictionary<string, int[][]> creatures = [];
    private readonly Dictionary<string, int> tints = [];

    public int[][] Creature(string name) => creatures.TryGetValue(name, out var c) ? c : creatures[name] = Core?.Creature(name) ?? [];

    /// A model keeps one colour everywhere. Suggestions take turns, so neighbours differ.
    public System.Windows.Media.SolidColorBrush Tint(string name)
    {
        var n = tints.TryGetValue(name, out var t) ? t : Creature(name).SelectMany(r => r).Aggregate(0, (a, r) => (a * 31 + r) & 0xFFFF);
        return Theme.Tints[n % Theme.Tints.Length];
    }
    private void AssignTints(Shortlist s)
    {
        tints.Clear();
        foreach (var c in s.Picks.Select(p => p.Choice).Concat(s.Ranked).Concat(s.Unrated)) tints.TryAdd(c.Name, tints.Count);
    }

    // Browse

    public async void Search(string query, bool more = false)
    {
        var turn = ++searchTurn;
        Query = query;
        if (!more && query != "") await Task.Delay(260);
        if (turn != searchTurn) return;
        (Searching, SearchError) = (true, null);
        Say("search");
        try
        {
            var page = await Core!.Call<Page>("search", more ? new { q = query, cursor = Next } : new { q = query, cursor = (string?)null });
            if (turn != searchTurn) return;
            if (more)
            {
                var known = Hits.Select(h => h.Id).ToHashSet();
                Hits.AddRange(page.Hits.Where(h => !known.Contains(h.Id)));
            }
            else Hits = page.Hits;
            (Total, Next) = (page.Total, page.Next);
        }
        catch (Exception e) { if (turn == searchTurn) SearchError = e.Message; }
        Searching = false;
        Say("search");
    }

    // Library

    public Item? Find(string repo, List<RemoteFile> files) => files.Count == 0 ? null : Items.FirstOrDefault(i => i.Key == $"{repo}/{files[0].Path}");
    public async void Download(string id, List<RemoteFile> files) => await Do("download", new { id, paths = files.Select(f => f.Path) });
    public async void Act(string method, string key) => await Do(method, new { key });
    public async void Reveal(string key)
    {
        if (await Try(() => Core!.Call<string>("path", new { key })) is { } p)
        {
            var start = new System.Diagnostics.ProcessStartInfo("explorer.exe");
            start.ArgumentList.Add("/select," + p);
            System.Diagnostics.Process.Start(start);
        }
    }
    public async void Measure(string key)
    {
        if (ChatKey is not null) CloseChat();
        Measuring.Add(key);
        Say("items");
        if (!await Do("measure", new { key })) { Measuring.Remove(key); Say("items"); }
    }
    public async void MoveModels(string dir)
    {
        if (await Do("settings.set", new { models_dir = dir })) Notify($"New downloads will go to {dir} after you reopen RunIt.");
    }

    // Chat

    public async void OpenChat(string key)
    {
        Turns.Clear();
        (ChatKey, LiveSpeed, ChatError, ChatNote, ChatLoading, ChatBusy) = (key, 0, null, null, true, false);
        Say("place");
        try { await Core!.Run("chat.open", new { key }); } catch (Exception e) { if (ChatKey == key) ChatError = e.Message; }
        if (ChatKey == key) ChatLoading = false;
        Say("chat");
    }
    public async void CloseChat()
    {
        (ChatKey, ChatBusy) = (null, false);
        Say("place");
        try { await Core!.Run("chat.close"); } catch (CoreException) { }
    }
    public async void Send(string text)
    {
        if (ChatBusy) return;
        (ChatBusy, ChatError) = (true, null);
        Turns.Add(new(true, text));
        var messages = Turns.Select(t => new { role = t.Mine ? "user" : "assistant", content = t.Text }).ToList();
        Turns.Add(new(false, ""));
        Say("chat");
        try { await Core!.Run("chat.send", new { messages }); } catch (Exception e) { (ChatBusy, ChatError) = (false, e.Message); Say("chat"); }
    }
    public async void StopReply() => await Do("chat.stop");

    private async void Handle(Event e)
    {
        switch (e.Type, e.Kind)
        {
            case ("bench", "stage"): (Stage, Progress) = (e.Text ?? "", e.Progress ?? Progress); Say("test"); break;
            case ("bench", "finding"): if (e.Text is { } t) Lines.Add(t); Progress = e.Progress ?? Progress; Say("line"); break;
            case ("bench", "done"):
                (Report, Progress, Testing, Shortlist) = (e.Report, 1, false, null);
                Say("test");
                await Recommend();
                break;
            case ("bench", "stopped"): Testing = false; Lines.Clear(); Entered = Report is not null; Say("shell"); break;
            case ("bench", "failed"): (Testing, TestError) = (false, e.Error); Say("test"); break;
            case ("catalog", _): CatalogProgress = e.Progress; Say("catalog"); break;
            case ("item", _):
                if (e.Removed is { } gone) Items.RemoveAll(i => i.Key == gone);
                if (e.Item is { } item) { var n = Items.FindIndex(i => i.Key == item.Key); if (n >= 0) Items[n] = item; else Items.Insert(0, item); }
                Say("items");
                break;
            case ("measure", "done"): Measuring.Remove(e.Key ?? ""); Say("items"); break;
            case ("measure", "failed"): Measuring.Remove(e.Key ?? ""); Notify(e.Error); Say("items"); break;
            case ("chat", _) when e.Key == ChatKey:
                switch (e.Kind)
                {
                    case "text" when Turns.Count > 0: Turns[^1].Text += e.Text; break;
                    case "thought" when Turns.Count > 0: Turns[^1].Thought += e.Text; break;
                    case "speed": LiveSpeed = e.TokensPerS ?? LiveSpeed; break;
                    case "end": (ChatBusy, ChatNote) = (false, e.Text); break;
                    case "failed": (ChatBusy, ChatError) = (false, e.Error); break;
                }
                Say(e.Kind is "end" or "failed" ? "chat" : "reply");
                break;
        }
    }
}
