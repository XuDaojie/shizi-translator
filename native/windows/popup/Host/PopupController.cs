using Microsoft.UI.Dispatching;

namespace Shizi.Popup.Host;

/// <summary>
/// 窗口壳操作，语义对齐 C ABI：
/// shizi_popup_ensure / show / hide / set_always_on_top / set_size / shutdown / is_available。
/// </summary>
public sealed class PopupController
{
    private readonly MainWindow _window;
    private readonly DispatcherQueue _queue;
    private bool _ensured;

    public PopupController(MainWindow window)
    {
        _window = window;
        _queue = window.DispatcherQueue;
    }

    public MainWindow Window => _window;

    public bool IsAvailable => true;

    public void Ensure()
    {
        RunOnUi(() =>
        {
            // 窗体已在 App 中创建；ensure = 保证隐藏窗存在
            if (!_window.IsPopupVisible)
            {
                _window.HidePopup();
            }
            _ensured = true;
        });
    }

    /// <param name="mode">0 NearCursor / 1 Restore</param>
    public void Show(double x, double y, int mode)
    {
        RunOnUi(() =>
        {
            if (!_ensured)
            {
                _ensured = true;
            }
            _window.ShowPopup(x, y, mode);
        });
    }

    public void Hide()
    {
        RunOnUi(() => _window.HidePopup());
    }

    public void SetAlwaysOnTop(bool on)
    {
        RunOnUi(() => _window.SetAlwaysOnTop(on));
    }

    public void SetSize(double width, double height)
    {
        RunOnUi(() => _window.SetSizeLogical(width, height));
    }

    public void Shutdown()
    {
        RunOnUi(() =>
        {
            _window.HidePopup();
            _ensured = false;
        });
    }

    private void RunOnUi(Action action)
    {
        if (_queue.HasThreadAccess)
        {
            action();
            return;
        }

        var done = new ManualResetEventSlim(false);
        Exception? error = null;
        _ = _queue.TryEnqueue(() =>
        {
            try
            {
                action();
            }
            catch (Exception ex)
            {
                error = ex;
            }
            finally
            {
                done.Set();
            }
        });
        // IPC 线程同步等待 UI 完成（超时防死锁）
        if (!done.Wait(TimeSpan.FromSeconds(5)))
        {
            throw new TimeoutException("UI 调度超时");
        }
        if (error is not null)
        {
            throw error;
        }
    }
}
