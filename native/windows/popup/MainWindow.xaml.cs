using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Windows.Graphics;
using WinRT.Interop;

namespace Shizi.Popup;

public sealed partial class MainWindow : Window
{
    private readonly AppWindow _appWindow;
    private bool _alwaysOnTop;

    public MainWindow()
    {
        InitializeComponent();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        var hwnd = WindowNative.GetWindowHandle(this);
        var windowId = Win32Interop.GetWindowIdFromWindow(hwnd);
        _appWindow = AppWindow.GetFromWindowId(windowId);

        // 不进任务栏 / Alt-Tab 切换列表
        _appWindow.IsShownInSwitchers = false;

        if (_appWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
            presenter.SetBorderAndTitleBar(true, false);
        }

        // 默认逻辑尺寸（约 420 宽）
        TryResizeLogical(420, 200);

        // 关窗 = hide，不销毁
        _appWindow.Closing += (_, e) =>
        {
            e.Cancel = true;
            HidePopup();
        };

        // 初始隐藏
        _appWindow.Hide();
    }

    public bool IsPopupVisible { get; private set; }

    public void ShowPopup(double logicalX, double logicalY, int mode)
    {
        // mode 0 NearCursor：用坐标；1 Restore：保留当前位置
        if (mode == 0)
        {
            MoveLogical(logicalX, logicalY);
        }

        _appWindow.Show();
        // 尽量前置
        try
        {
            if (_appWindow.Presenter is OverlappedPresenter p)
            {
                p.IsAlwaysOnTop = _alwaysOnTop;
            }
        }
        catch
        {
            // ignore
        }

        IsPopupVisible = true;
        Activate();
    }

    public void HidePopup()
    {
        _appWindow.Hide();
        IsPopupVisible = false;
    }

    public void SetAlwaysOnTop(bool on)
    {
        _alwaysOnTop = on;
        if (_appWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsAlwaysOnTop = on;
        }
    }

    public void SetSizeLogical(double width, double height)
    {
        TryResizeLogical(width, height);
    }

    public void MoveLogical(double x, double y)
    {
        var scale = Content?.XamlRoot?.RasterizationScale ?? GetDpiScaleFallback();
        var px = (int)Math.Round(x * scale);
        var py = (int)Math.Round(y * scale);
        _appWindow.Move(new PointInt32(px, py));
    }

    private void TryResizeLogical(double width, double height)
    {
        var scale = Content?.XamlRoot?.RasterizationScale ?? GetDpiScaleFallback();
        var w = Math.Max(1, (int)Math.Round(width * scale));
        var h = Math.Max(1, (int)Math.Round(height * scale));
        _appWindow.Resize(new SizeInt32(w, h));
    }

    private static double GetDpiScaleFallback()
    {
        // 无 XamlRoot 时退回 1.0；ensure 后首帧再校正
        return 1.0;
    }

    /// <summary>任务 9/10：接收 Bridge push 时更新占位文案（最小反馈）。</summary>
    public void OnBridgePushHint(string typeName)
    {
        try
        {
            DispatcherQueue.TryEnqueue(() =>
            {
                TitleHint.Text = typeName;
            });
        }
        catch
        {
            // ignore
        }
    }
}
