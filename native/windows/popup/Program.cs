using System.Runtime.InteropServices;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Shizi.Popup.Host;
using WinRT;

namespace Shizi.Popup;

/// <summary>
/// 入口：解析 --port / --pipe 后启动 WinUI STA 消息循环。
/// 当前 transport = 子进程 + localhost TCP（见 native/README.md）。
/// </summary>
public static class Program
{
    internal static HostOptions StartupOptions { get; private set; } =
        new HostOptions(Port: null, PipeName: null);

    [DllImport("Microsoft.ui.xaml.dll")]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    private static extern void XamlCheckProcessRequirements();

    [STAThread]
    public static void Main(string[] args)
    {
        // 捕获未处理异常，避免「翻译一触发进程直接没了」却无日志
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
        {
            if (e.ExceptionObject is Exception ex)
                CrashLog.Write("AppDomain.UnhandledException", ex);
            else
                CrashLog.Write($"AppDomain.UnhandledException: {e.ExceptionObject}");
        };
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            CrashLog.Write("TaskScheduler.UnobservedTaskException", e.Exception);
            e.SetObserved();
        };

        try
        {
            XamlCheckProcessRequirements();
        }
        catch
        {
            // SelfContained 下通常仍可继续
        }

        ComWrappersSupport.InitializeComWrappers();

        StartupOptions = HostOptions.Parse(args)
            ?? new HostOptions(Port: null, PipeName: null);

        // SelfContained：WindowsAppSdkBootstrapInitialize=false，依赖旁路 WASDK DLL。
        Application.Start(OnApplicationStart);
    }

    private static void OnApplicationStart(ApplicationInitializationCallbackParams args)
    {
        _ = args;
        var context = new DispatcherQueueSynchronizationContext(
            DispatcherQueue.GetForCurrentThread());
        SynchronizationContext.SetSynchronizationContext(context);
        new App();
    }
}

/// <summary>命令行：--port N 或 --pipe name（管道名不含 \\.\pipe\ 前缀）。</summary>
public sealed record HostOptions(int? Port, string? PipeName)
{
    public static HostOptions? Parse(string[] args)
    {
        int? port = null;
        string? pipe = null;
        for (var i = 0; i < args.Length; i++)
        {
            var a = args[i];
            if ((a == "--port" || a == "-p") && i + 1 < args.Length
                && int.TryParse(args[i + 1], out var p))
            {
                port = p;
                i++;
            }
            else if (a == "--pipe" && i + 1 < args.Length)
            {
                pipe = args[i + 1];
                i++;
            }
        }

        if (port is null && string.IsNullOrWhiteSpace(pipe))
        {
            return null;
        }

        return new HostOptions(port, pipe);
    }
}
