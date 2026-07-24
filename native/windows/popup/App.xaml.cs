using Microsoft.UI.Xaml;
using Shizi.Popup.Bridge;
using Shizi.Popup.Host;

namespace Shizi.Popup;

public partial class App : Application
{
    private readonly HostOptions _options;
    private Window? _window;
    private PopupController? _controller;
    private IpcHost? _ipc;

    public App()
    {
        _options = Program.StartupOptions;
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _window = new MainWindow();
        _controller = new PopupController((MainWindow)_window);
        NativeBridge.Instance.Attach(_controller);

        // 建隐藏窗（ensure 语义）；独立启动时也可 show 看壳
        _controller.Ensure();

        if (_options.Port is int port)
        {
            _ipc = IpcHost.StartTcp(port, _controller, NativeBridge.Instance);
        }
        else if (!string.IsNullOrWhiteSpace(_options.PipeName))
        {
            _ipc = IpcHost.StartNamedPipe(_options.PipeName!, _controller, NativeBridge.Instance);
        }
        else
        {
            // 无 IPC：展示占位壳，便于手工验收
            _controller.Show(200, 200, mode: 1);
        }
    }

    public void RequestShutdown()
    {
        try
        {
            _ipc?.Dispose();
        }
        catch
        {
            // best-effort
        }

        try
        {
            _controller?.Shutdown();
        }
        catch
        {
            // best-effort
        }

        try
        {
            Exit();
        }
        catch
        {
            // ignore
        }

        // WinUI Exit 在有后台 IPC 线程时可能不立刻结束进程
        Environment.Exit(0);
    }
}
