using System.Windows;

namespace RunIt;

public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        DispatcherUnhandledException += (_, x) =>
        {
            if (Rehearsal.Folder is not null) Rehearsal.Fail(x.Exception.ToString());
            Store.Current.Notify(x.Exception.Message);
            x.Handled = true;
        };
        Theme.Apply();
        MainWindow = new Shell();
        MainWindow.Show();
        Store.Current.Start();
        Rehearsal.Begin();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        Store.Current.Core?.Dispose();
        base.OnExit(e);
    }
}
