using System.Runtime.InteropServices;
using Microsoft.UI;
using Microsoft.UI.Composition;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Shapes;
using Shizi.Popup.Data;
using Shizi.Popup.Services;
using Shizi.Popup.State;
using Windows.ApplicationModel.DataTransfer;
using Windows.Graphics;
using Windows.System;
using Windows.UI;
using WinRT;
using WinRT.Interop;

namespace Shizi.Popup;

public sealed partial class MainWindow : Window
{
    private static readonly IntPtr HwndTopmost = new(-1);
    private static readonly IntPtr HwndNoTopmost = new(-2);
    private const uint SwpNomove = 0x0002;
    private const uint SwpNosize = 0x0001;
    private const uint SwpShowwindow = 0x0040;
    private const uint SwpNoactivate = 0x0010;

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool BringWindowToTop(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool SetWindowPos(
        IntPtr hWnd,
        IntPtr hWndInsertAfter,
        int x,
        int y,
        int cx,
        int cy,
        uint uFlags);

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

    // DWMWA_WINDOW_CORNER_PREFERENCE = 33; DWMWCP_ROUND = 2; DWMWCP_ROUNDSMALL = 3
    private const int DwmwaWindowCornerPreference = 33;
    private const int DwmwcpRound = 2;

    private readonly AppWindow _appWindow;
    private readonly BridgeService _bridge = BridgeService.Instance;
    private readonly SpeechService _speech = new();
    private bool _alwaysOnTop;
    private bool _suppressSourceTextEvents;
    private bool _startupStarted;
    private bool _sizeReportBusy;
    private double _lastReportedHeight;
    private DispatcherTimer? _tipTimer;
    private DispatcherTimer? _sizeDebounce;
    private DispatcherTimer? _sizeFollowTimer;
    private int _sizeFollowRemaining;
    private Storyboard? _statusDotPulse;
    private SystemBackdropConfiguration? _micaConfig;
    /// <summary>卡片 loading 脉冲/光标闪烁等 Forever Storyboard；Rebuild 前必须 Stop，否则目标被拆掉会崩进程。</summary>
    private readonly List<Storyboard> _cardStoryboards = new();
    /// <summary>合并高频 RaiseUi（delta 流）为单次 UI 刷新，降低 Clear/Storyboard 抖动。</summary>
    private bool _uiRefreshQueued;

    /// <summary>与 Vue components.css 卡片折叠 / 展开全文 transition 对齐（0.15s）。</summary>
    private const int CardAnimMs = 150;
    /// <summary>正文折叠上限 6.4em @ 13px line-height 1.6 → 约 4 行。</summary>
    private const double ResultClipCollapsedPx = 20.8 * 4;
    /// <summary>
    /// 原型滑块「Mica 透明度」80% → CSS alpha=0.80。
    /// 系统 Acrylic 比 CSS backdrop-filter 更密，Tint 略低于 0.80 才能在观感上对齐原型 80%（否则像 95~98% 实色）。
    /// </summary>
    private const float PrototypeMicaOpacity = 0.55f;
    private const float PrototypeMicaLuminosity = 0.65f;
    private DesktopAcrylicController? _acrylicController;
    /// <summary>当前材质路径，写进托盘/钉 Tooltip，便于确认不是旧 exe。</summary>
    private string _backdropMode = "unset";

    public MainWindow()
    {
        InitializeComponent();
        ApplyUnifiedTypography();

        // 仅中间 DragRegion（AppTitleBar）参与拖拽；左右按钮在其外，可正常点击。
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        var hwnd = WindowNative.GetWindowHandle(this);
        var windowId = Win32Interop.GetWindowIdFromWindow(hwnd);
        _appWindow = AppWindow.GetFromWindowId(windowId);

        _appWindow.IsShownInSwitchers = false;

        ConfigurePopupPresenter();
        TryApplyRoundedCorners(hwnd);
        TryApplyMicaBackdrop();

        // 双击标题拖拽区仍可能被系统最大化：强制还原，弹窗不允许最大化/全屏。
        _appWindow.Changed += AppWindow_Changed;

        TryResizeLogical(PopupLogicalWidth, 280);

        _appWindow.Closing += (_, e) =>
        {
            e.Cancel = true;
            HidePopup();
        };

        _appWindow.Hide();

        InitLanguageCombos();
        ApplyLocalizedChrome();
        RefreshFromBridge();

        _bridge.UiChanged += OnBridgeUiChanged;
        _bridge.LocaleChanged += OnLocaleChanged;

        // 首帧后跑 ready / config / pending
        Activated += MainWindow_FirstActivated;
    }

    /// <summary>弹窗逻辑宽（对齐原型 468）。</summary>
    private const double PopupLogicalWidth = 468;

    /// <summary>
    /// 统一字族/字号：原文 TextBox、标题栏、语言栏、状态栏与结果卡共用 PopupFontFamily + 13/12/11 阶梯。
    /// 在 code-behind 赋值，避免 XAML TemplateBinding 某些组合触发 XamlCompiler 静默失败。
    /// </summary>
    private void ApplyUnifiedTypography()
    {
        var font = PopupFont();
        var body = PopupFontSize("PopupFontSizeBody", 13);
        var caption = PopupFontSize("PopupFontSizeCaption", 12);
        var badge = PopupFontSize("PopupFontSizeBadge", 11);

        // Grid 无 FontFamily；只设 Control/TextBlock 系
        if (SourceTextBox is not null)
        {
            SourceTextBox.FontFamily = font;
            SourceTextBox.FontSize = body;
        }

        void SetTb(TextBlock? tb, double size)
        {
            if (tb is null)
                return;
            tb.FontFamily = font;
            tb.FontSize = size;
        }

        SetTb(SourceLangLabel, body);
        SetTb(TargetLangLabel, body);
        SetTb(SourceLangBadge, badge);
        SetTb(SourceTypeBadge, badge);
        SetTb(StatusText, caption);
        SetTb(CharCountText, caption);
        if (StatusActionButton is not null)
        {
            StatusActionButton.FontFamily = font;
            StatusActionButton.FontSize = caption;
        }

        // 标题栏品牌字
        if (Content is FrameworkElement)
        {
            // brand name / 文标在 XAML 内联，遍历 Shell 内 TextBlock 补字族（字号已对齐）
            ApplyFontFamilyRecursive(ShellBorder, font);
        }
    }

    private static void ApplyFontFamilyRecursive(DependencyObject? root, FontFamily font)
    {
        if (root is null)
            return;
        if (root is TextBlock tb)
            tb.FontFamily = font;
        else if (root is Control ctl && root is not Button) // Button 内容另有 FontIcon
            ctl.FontFamily = font;

        var count = VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
            ApplyFontFamilyRecursive(VisualTreeHelper.GetChild(root, i), font);
    }

    /// <summary>无边框浮窗：不可改大小 / 最大化 / 最小化；自绘圆角 shell。</summary>
    private void ConfigurePopupPresenter()
    {
        if (_appWindow.Presenter is not OverlappedPresenter presenter)
        {
            return;
        }

        presenter.IsResizable = false;
        presenter.IsMaximizable = false;
        presenter.IsMinimizable = false;
        // 去掉系统标题栏与厚边框，才能接近原型「圆角 Mica 卡片」
        presenter.SetBorderAndTitleBar(false, false);
        if (presenter.State == OverlappedPresenterState.Maximized
            || presenter.State == OverlappedPresenterState.Minimized)
        {
            presenter.Restore();
        }
    }

    /// <summary>Win11 圆角（DWM）；失败则仍靠 XAML CornerRadius 裁剪内容。</summary>
    private static void TryApplyRoundedCorners(IntPtr hwnd)
    {
        try
        {
            var pref = DwmwcpRound;
            _ = DwmSetWindowAttribute(hwnd, DwmwaWindowCornerPreference, ref pref, sizeof(int));
        }
        catch
        {
            // ignore
        }
    }

    /// <summary>
    /// 对齐原型：rgba(244,244,244,0.80) + backdrop-filter blur。
    /// 优先 SystemBackdrop API（比手动 Controller 更不易静默失败落到实色）。
    /// Shell 必须 Transparent，否则会盖住材质变成「实灰板」。
    /// </summary>
    private void TryApplyMicaBackdrop()
    {
        RootGrid.Background = new SolidColorBrush(Colors.Transparent);
        // 关键：XAML 默认 PopupMicaFallbackBrush 是实色灰；不先清掉则永远像 98% 实色
        ShellBorder.Background = new SolidColorBrush(Colors.Transparent);

        // 1) 自定义 Acrylic（可控 Tint≈原型 alpha）
        if (TryApplyCustomAcrylic())
        {
            SetBackdropMode("acrylic-custom");
            return;
        }

        // 2) 系统默认 DesktopAcrylic（保证至少有 blur/透）
        try
        {
            if (DesktopAcrylicController.IsSupported())
            {
                SystemBackdrop = new DesktopAcrylicBackdrop();
                SetBackdropMode("acrylic-default");
                return;
            }
        }
        catch (Exception ex)
        {
            CrashLog.Write("DesktopAcrylicBackdrop", ex);
        }

        // 3) Mica BaseAlt
        try
        {
            if (MicaController.IsSupported())
            {
                SystemBackdrop = new MicaBackdrop { Kind = MicaKind.BaseAlt };
                SetBackdropMode("mica-basealt");
                return;
            }
        }
        catch (Exception ex)
        {
            CrashLog.Write("MicaBackdrop", ex);
        }

        // 4) 最后才 solid（无 blur，观感会差）
        var a = (byte)Math.Clamp((int)(0.80f * 255f), 0, 255);
        ShellBorder.Background = new SolidColorBrush(Color.FromArgb(a, 0xF4, 0xF4, 0xF4));
        SetBackdropMode("solid-fallback");
    }

    private bool TryApplyCustomAcrylic()
    {
        try
        {
            if (!DesktopAcrylicController.IsSupported())
                return false;

            EnsureBackdropConfig();
            // 先清掉可能冲突的 SystemBackdrop
            SystemBackdrop = null;

            _acrylicController?.Dispose();
            _acrylicController = new DesktopAcrylicController
            {
                TintColor = Color.FromArgb(255, 0xF4, 0xF4, 0xF4),
                // 观感对齐原型 80% 滑块（系统材质更密，Tint 取 0.55）
                TintOpacity = PrototypeMicaOpacity,
                LuminosityOpacity = PrototypeMicaLuminosity,
            };
            _acrylicController.AddSystemBackdropTarget(this.As<ICompositionSupportsSystemBackdrop>());
            _acrylicController.SetSystemBackdropConfiguration(_micaConfig!);
            return true;
        }
        catch (Exception ex)
        {
            CrashLog.Write("TryApplyCustomAcrylic", ex);
            try
            {
                _acrylicController?.Dispose();
            }
            catch
            {
                // ignore
            }

            _acrylicController = null;
            return false;
        }
    }

    private void SetBackdropMode(string mode)
    {
        _backdropMode = mode;
        try
        {
            CrashLog.Write($"backdrop mode={mode} tint={PrototypeMicaOpacity:0.00} lum={PrototypeMicaLuminosity:0.00}");
            // 钉按钮 Tooltip 带模式，便于确认不是旧进程
            if (PinButton is not null)
            {
                ToolTipService.SetToolTip(
                    PinButton,
                    $"{Localization.T(_alwaysOnTop ? "popup.tooltip.unpin" : "popup.tooltip.pin")} · {mode}");
            }

            Title = $"Shizi · {mode}";
        }
        catch
        {
            // ignore
        }
    }

    private void EnsureBackdropConfig()
    {
        if (_micaConfig is not null)
            return;

        // 始终 InputActive：失焦时系统会把 Acrylic/Mica 切到 inactive 极透态
        _micaConfig = new SystemBackdropConfiguration { IsInputActive = true };
        ApplyBackdropTheme();

        Activated += OnWindowActivatedForBackdrop;
        if (Content is FrameworkElement rootFe)
            rootFe.ActualThemeChanged += (_, _) => ApplyBackdropTheme();
    }

    /// <summary>失焦不 hide；材质保持 active 外观。</summary>
    private void OnWindowActivatedForBackdrop(object sender, WindowActivatedEventArgs args)
    {
        // 产品：丢失焦点不自动隐藏（仅关闭/最小化/截图译路径 hide）
        if (_micaConfig is not null)
            _micaConfig.IsInputActive = true;
        _ = args;
    }

    private void ApplyBackdropTheme()
    {
        if (_micaConfig is null)
            return;
        var theme = (Content as FrameworkElement)?.ActualTheme ?? ElementTheme.Default;
        _micaConfig.Theme = theme switch
        {
            ElementTheme.Dark => SystemBackdropTheme.Dark,
            ElementTheme.Light => SystemBackdropTheme.Light,
            _ => SystemBackdropTheme.Default,
        };
    }

    private void AppWindow_Changed(AppWindow sender, AppWindowChangedEventArgs args)
    {
        if (!args.DidPresenterChange && !args.DidSizeChange)
        {
            return;
        }

        // 标题栏双击等路径仍可能进入 Maximized；立刻还原并重申约束。
        ConfigurePopupPresenter();
    }

    private void MainWindow_FirstActivated(object sender, WindowActivatedEventArgs args)
    {
        if (_startupStarted)
            return;
        _startupStarted = true;
        Activated -= MainWindow_FirstActivated;

        _ = DispatcherQueue.TryEnqueue(async () =>
        {
            try
            {
                await _bridge.RunStartupSequenceAsync().ConfigureAwait(true);
            }
            catch
            {
                // best-effort
            }
            finally
            {
                RefreshFromBridge();
                ReportContentSize();
            }
        });
    }

    public bool IsPopupVisible { get; private set; }

    public void ShowPopup(double logicalX, double logicalY, int mode)
    {
        if (mode == 0)
        {
            MoveLogical(logicalX, logicalY);
        }

        // show 前后重申 presenter：避免上次误最大化后仍以全屏形态出现。
        ConfigurePopupPresenter();
        _appWindow.Show();
        IsPopupVisible = true;
        ConfigurePopupPresenter();

        // 快捷键/托盘在 shizi 主进程触发，弹窗在子进程：Windows 会拦截跨进程
        // SetForegroundWindow/Activate。结果是「IPC show 成功、IsWindowVisible=true」，
        // 但窗体被 IDEA/Chrome 等盖住，用户感觉「打不开」。先抬 z-order 再尝试激活。
        BringPopupToFront();

        Activate();
        // 不要在 IPC 同步 Show 路径内立刻 report_content_size：
        // 否则 request 可能先于 show 的 result 到达 Rust，嵌套 set_size 与
        // 外层 show 的 OP_GATE 形成死锁/超时，导致 Rust 回退 webview。
        _ = DispatcherQueue.TryEnqueue(() => ReportContentSize());
    }

    /// <summary>
    /// 强制把弹窗抬到可见 z-order 并尽量抢前台。
    /// 临时 TOPMOST → 再按钉住状态恢复，避免仅 Activate 被静默忽略。
    /// </summary>
    private void BringPopupToFront()
    {
        try
        {
            var hwnd = WindowNative.GetWindowHandle(this);

            // 1) Presenter 临时置顶（即使失败也不影响后续 Win32）
            if (_appWindow.Presenter is OverlappedPresenter presenter)
            {
                presenter.IsAlwaysOnTop = true;
            }

            // 2) Win32：TOPMOST 抬 z-order，再尝试前台
            _ = SetWindowPos(
                hwnd,
                HwndTopmost,
                0,
                0,
                0,
                0,
                SwpNomove | SwpNosize | SwpShowwindow);
            _ = BringWindowToTop(hwnd);
            _ = SetForegroundWindow(hwnd);

            // 3) 用户未钉住则撤掉 TOPMOST（保留当前相对 z-order 靠前）
            if (!_alwaysOnTop)
            {
                if (_appWindow.Presenter is OverlappedPresenter p)
                {
                    p.IsAlwaysOnTop = false;
                }

                _ = SetWindowPos(
                    hwnd,
                    HwndNoTopmost,
                    0,
                    0,
                    0,
                    0,
                    SwpNomove | SwpNosize | SwpNoactivate);
            }
        }
        catch
        {
            // best-effort：前台限制失败时至少窗体已 Show
        }
    }

    public void HidePopup()
    {
        _speech.Stop();
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

        UpdatePinButtonVisual();
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

    /// <summary>Bridge push 时可选状态提示（调试/状态条旁）。</summary>
    public void OnBridgePushHint(string typeName)
    {
        _ = typeName;
        // 完整 UI 已绑定 BridgeService；保留空钩子以免 Host 调用失败
    }

    private void OnBridgeUiChanged()
    {
        try
        {
            // 已在 UI 线程则合并到下一帧；否则入队一次，期间多次 RaiseUi 只刷新一次
            if (DispatcherQueue.HasThreadAccess)
            {
                if (_uiRefreshQueued)
                    return;
                _uiRefreshQueued = true;
                _ = DispatcherQueue.TryEnqueue(() =>
                {
                    _uiRefreshQueued = false;
                    SafeRefreshFromBridge();
                });
                return;
            }

            if (_uiRefreshQueued)
                return;
            _uiRefreshQueued = true;
            if (!DispatcherQueue.TryEnqueue(() =>
                {
                    _uiRefreshQueued = false;
                    SafeRefreshFromBridge();
                }))
            {
                _uiRefreshQueued = false;
            }
        }
        catch (Exception ex)
        {
            _uiRefreshQueued = false;
            CrashLog.Write("OnBridgeUiChanged.Enqueue", ex);
        }
    }

    private void SafeRefreshFromBridge()
    {
        try
        {
            RefreshFromBridge();
            ReportContentSize();
        }
        catch (Exception ex)
        {
            CrashLog.Write("OnBridgeUiChanged.Refresh", ex);
        }
    }

    private void OnLocaleChanged()
    {
        try
        {
            DispatcherQueue.TryEnqueue(ApplyLocalizedChrome);
        }
        catch
        {
            // ignore
        }
    }

    private void InitLanguageCombos()
    {
        SourceLangList.ItemsSource = TranslationLanguages.All.ToList();
        TargetLangList.ItemsSource = TranslationLanguages.Targets.ToList();
        RefreshLanguageLabels();
    }

    private void RefreshLanguageLabels()
    {
        SourceLangLabel.Text = TranslationLanguages.DisplayName(_bridge.SessionSourceLang);
        if (string.IsNullOrEmpty(SourceLangLabel.Text))
            SourceLangLabel.Text = TranslationLanguages.Auto.NativeName;
        TargetLangLabel.Text = TranslationLanguages.DisplayName(_bridge.SessionTargetLang);
        if (string.IsNullOrEmpty(TargetLangLabel.Text))
            TargetLangLabel.Text = "简体中文";
    }

    private void ApplyLocalizedChrome()
    {
        Title = Localization.T("window.popupTitle");
        SourceTextBox.PlaceholderText = Localization.T("popup.source.placeholder");
        ToolTipService.SetToolTip(PinButton, Localization.T(_alwaysOnTop ? "popup.tooltip.unpin" : "popup.tooltip.pin"));
        ToolTipService.SetToolTip(OcrButton, Localization.T("popup.tooltip.ocr"));
        ToolTipService.SetToolTip(SettingsButton, Localization.T("popup.tooltip.settings"));
        ToolTipService.SetToolTip(FavButton, Localization.T("popup.tooltip.bookmark"));
        ToolTipService.SetToolTip(BookmarkButton, Localization.T("popup.tooltip.bookmark"));
        ToolTipService.SetToolTip(ThemeButton, Localization.T("popup.tooltip.theme"));
        ToolTipService.SetToolTip(MinButton, Localization.T("popup.tooltip.minimize"));
        ToolTipService.SetToolTip(CloseButton, Localization.T("popup.tooltip.close"));
        RefreshStatusChrome();
        UpdateCharCount();
    }

    private void RefreshFromBridge()
    {
        // 原文
        if (!_suppressSourceTextEvents && SourceTextBox.Text != _bridge.SourceText)
        {
            _suppressSourceTextEvents = true;
            try
            {
                SourceTextBox.Text = _bridge.SourceText;
            }
            finally
            {
                _suppressSourceTextEvents = false;
            }
        }

        // 语言标签
        RefreshLanguageLabels();

        // 源语徽章
        if (_bridge.SessionSourceLang == "auto")
        {
            SourceLangBadge.Text = string.IsNullOrEmpty(_bridge.DetectedLangBadge)
                ? (_bridge.IsTranslating
                    ? Localization.T("popup.status.detecting")
                    : TranslationLanguages.Auto.NativeName)
                : TranslationLanguages.DisplayName(_bridge.DetectedLangBadge);
        }
        else
        {
            SourceLangBadge.Text = TranslationLanguages.DisplayName(_bridge.SessionSourceLang);
        }

        if (_bridge.SourceBadge is { } badge)
        {
            SourceTypeBadgeBorder.Visibility = Visibility.Visible;
            SourceTypeBadge.Text = Localization.T($"popup.badge.{badge}");
        }
        else
        {
            SourceTypeBadgeBorder.Visibility = Visibility.Collapsed;
        }

        RebuildResultCards();
        RefreshStatusChrome();
        UpdateCharCount();
        UpdatePinButtonVisual();

        if (!string.IsNullOrEmpty(_bridge.TipMessage))
        {
            ShowTipBar(_bridge.TipMessage);
            _bridge.ClearTip();
        }
    }

    private void RefreshStatusChrome()
    {
        StatusText.Text = Localization.T(_bridge.StatusKey);

        // 状态点：翻译中 accent + pulse；完成 success；失败 destructive；其它 accent 静态
        var accent = TryResourceBrush("PopupAccentBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0xD5, 0x5A, 0x1F));
        var success = TryResourceBrush("PopupSuccessBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0x10, 0x7C, 0x10));
        var danger = TryResourceBrush("PopupDestructiveBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0xC4, 0x2B, 0x1C));
        if (_bridge.StatusLoading)
            StatusDot.Fill = accent;
        else if (_bridge.StatusKey is "popup.status.failed" or "popup.status.partial")
            StatusDot.Fill = danger;
        else if (_bridge.StatusKey is "popup.status.completed")
            StatusDot.Fill = success;
        else
            StatusDot.Fill = accent;

        SetLoadingPulse(StatusDot, ref _statusDotPulse, _bridge.StatusLoading);

        switch (_bridge.StatusAction)
        {
            case StatusActionKind.Cancel:
                StatusActionButton.Visibility = Visibility.Visible;
                StatusActionButton.Content = Localization.T("popup.action.cancel");
                break;
            case StatusActionKind.Retry:
                StatusActionButton.Visibility = Visibility.Visible;
                StatusActionButton.Content = Localization.T("popup.action.retry");
                break;
            default:
                StatusActionButton.Visibility = Visibility.Collapsed;
                break;
        }
    }

    private void RebuildResultCards()
    {
        // 翻译 delta 会极高频 Rebuild：先停 Forever 动画再 Clear，避免目标已出树仍被 Composition 驱动而崩进程
        StopCardStoryboards();
        ResultsPanel.Children.Clear();

        // 快照，避免与状态机并发改动（push 已迁 UI 线程，仍防御）
        List<string> order;
        List<CardState> cards;
        try
        {
            order = _bridge.State.CardOrder.ToList();
            cards = order
                .Select(id => _bridge.State.Cards.TryGetValue(id, out var c) ? c : null)
                .Where(c => c is not null)
                .Cast<CardState>()
                .ToList();
        }
        catch (Exception ex)
        {
            CrashLog.Write("RebuildResultCards.Snapshot", ex);
            return;
        }

        foreach (var card in cards)
        {
            try
            {
                ResultsPanel.Children.Add(BuildResultCard(card));
            }
            catch (Exception ex)
            {
                CrashLog.Write($"RebuildResultCards.Build[{card.ServiceInstanceId}]", ex);
            }
        }
    }

    private void StopCardStoryboards()
    {
        foreach (var sb in _cardStoryboards)
        {
            try
            {
                sb.Stop();
            }
            catch
            {
                // ignore
            }
        }

        _cardStoryboards.Clear();
    }

    private void TrackCardStoryboard(Storyboard sb) => _cardStoryboards.Add(sb);

    private UIElement BuildResultCard(CardState card)
    {
        // 对齐 components.css .result-card / ResultCardView
        var cardBg = TryResourceBrush("PopupCardBgBrush")
            ?? new SolidColorBrush(Color.FromArgb(0xE6, 255, 255, 255));
        var borderBrush = TryResourceBrush("PopupBorderBrush")
            ?? new SolidColorBrush(Color.FromArgb(0x0F, 0, 0, 0));
        var border2 = TryResourceBrush("PopupBorder2Brush")
            ?? new SolidColorBrush(Color.FromArgb(0x1F, 0, 0, 0));
        var fg = TryResourceBrush("PopupFgBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0x1A, 0x1A, 0x1A));
        var fg2 = TryResourceBrush("PopupFg2Brush") ?? new SolidColorBrush(Color.FromArgb(255, 0x5D, 0x5D, 0x5D));
        var fg3 = TryResourceBrush("PopupFg3Brush") ?? new SolidColorBrush(Color.FromArgb(255, 0x8A, 0x8A, 0x8A));
        var accent = TryResourceBrush("PopupAccentBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0xD5, 0x5A, 0x1F));
        var danger = TryResourceBrush("PopupDestructiveBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0xC4, 0x2B, 0x1C));

        var root = new Border
        {
            Background = cardBg,
            BorderBrush = borderBrush,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
        };

        // 轻阴影感：hover 时加粗描边（原型 shadow-card-h）
        root.PointerEntered += (_, _) => root.BorderBrush = border2;
        root.PointerExited += (_, _) => root.BorderBrush = borderBrush;

        var stack = new StackPanel { Spacing = 0 };
        var serviceId = card.ServiceInstanceId;
        var displayName = string.IsNullOrWhiteSpace(card.ServiceName) ? card.ServiceInstanceId : card.ServiceName;

        // —— header: padding 6 12；icon 14 + name 11px + status + collapse 20 ——
        var header = new Grid { Padding = new Thickness(12, 6, 8, 6) };
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        // 与设置页 ServiceIcon 同源：Assets/service-icons/{serviceType}.svg
        var iconHost = new Border
        {
            Width = 14,
            Height = 14,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 6, 0),
            Child = ServiceIcons.Create(card.ServiceType, displayName, 14),
        };
        Grid.SetColumn(iconHost, 0);
        header.Children.Add(iconHost);

        var title = new TextBlock
        {
            Text = displayName,
            FontFamily = PopupFont(),
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, // 对齐 .result-engine-name 600
            FontSize = PopupFontSize("PopupFontSizeCaption", 12), // 0.75rem
            Foreground = fg2,
            VerticalAlignment = VerticalAlignment.Center,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        Grid.SetColumn(title, 1);
        header.Children.Add(title);

        // 状态点：对齐 Vue showDotFinal —— 仅 translating 显示 pulse；失败/取消红点；pending/完成不显示
        if (card.Status is CardStatus.Translating or CardStatus.Failed or CardStatus.Cancelled)
        {
            var fill = card.Status switch
            {
                CardStatus.Failed or CardStatus.Cancelled => danger,
                _ => accent,
            };
            var dot = new Ellipse
            {
                Width = 6,
                Height = 6,
                Fill = fill,
                Margin = new Thickness(2, 0, 4, 0),
                VerticalAlignment = VerticalAlignment.Center,
            };
            // 仅流式中脉冲（pending 空闲卡不转圈）
            if (card.Status == CardStatus.Translating)
                StartPulseForever(dot);
            Grid.SetColumn(dot, 2);
            header.Children.Add(dot);
        }

        // 折叠 chevron：始终向下，折叠时 RotateTransform -90°（对齐原型）
        var collapseIcon = new FontIcon
        {
            Glyph = "\uE70D",
            FontSize = PopupFontSize("PopupFontSizeBadge", 11),
            Foreground = fg2,
            RenderTransformOrigin = new Windows.Foundation.Point(0.5, 0.5),
            RenderTransform = new RotateTransform { Angle = card.Collapsed ? -90 : 0 },
        };
        var collapseBtn = new Button
        {
            Width = 20,
            Height = 20,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Colors.Transparent),
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(4),
            Content = collapseIcon,
        };
        if (Application.Current?.Resources.TryGetValue("PopupMetaButtonStyle", out var collapseStyleObj) == true
            && collapseStyleObj is Style collapseStyle)
        {
            collapseBtn.Style = collapseStyle;
            collapseBtn.Width = 20;
            collapseBtn.Height = 20;
            collapseBtn.Content = collapseIcon;
        }

        ToolTipService.SetToolTip(
            collapseBtn,
            Localization.T(card.Collapsed ? "popup.tooltip.expand" : "popup.tooltip.collapse"));
        Grid.SetColumn(collapseBtn, 3);
        header.Children.Add(collapseBtn);

        // body 始终在树内（MaxHeight 动画折叠），对齐 Vue grid 0fr↔1fr
        var body = BuildResultCardBody(card, serviceId, textSnapshot: card.Text ?? "", fg, fg2, fg3, accent, danger, borderBrush, out var textClip, out var expandChevron);
        // 展开态用大有限 MaxHeight（勿用 Infinity，否则 Measure 偶发污染窗高 → Resize 异常像「闪一下消失」）
        const double BodyExpandedMax = 8000;
        var bodyHost = new Border
        {
            Child = body,
            MaxHeight = card.Collapsed ? 0 : BodyExpandedMax,
            Opacity = card.Collapsed ? 0 : 1,
            // 裁剪子元素，避免折叠过程中内容溢出
            Clip = card.Collapsed
                ? new RectangleGeometry { Rect = new Windows.Foundation.Rect(0, 0, 0, 0) }
                : null,
        };

        void ToggleCollapse()
        {
            if (!_bridge.State.Cards.TryGetValue(serviceId, out var live))
                return;
            var collapsing = !live.Collapsed;
            _bridge.ToggleCardCollapsed(serviceId, raiseUi: false);
            ToolTipService.SetToolTip(
                collapseBtn,
                Localization.T(collapsing ? "popup.tooltip.expand" : "popup.tooltip.collapse"));
            AnimateCardBodyCollapse(bodyHost, body, collapseIcon, collapsing);
        }

        collapseBtn.Click += (_, _) => ToggleCollapse();
        header.PointerPressed += (_, e) =>
        {
            if (e.OriginalSource is DependencyObject src && IsDescendantOf(src, collapseBtn))
                return;
            ToggleCollapse();
        };

        stack.Children.Add(header);
        stack.Children.Add(bodyHost);
        root.Child = stack;
        return root;
    }

    /// <summary>构建结果卡 body（始终挂载，由 bodyHost MaxHeight 控制折叠）。</summary>
    private StackPanel BuildResultCardBody(
        CardState card,
        string serviceId,
        string textSnapshot,
        Brush fg,
        Brush fg2,
        Brush fg3,
        Brush accent,
        Brush danger,
        Brush borderBrush,
        out Border? textClip,
        out FontIcon? expandChevron)
    {
        textClip = null;
        expandChevron = null;

        var body = new StackPanel
        {
            Spacing = 0,
            Padding = new Thickness(12, 0, 12, 9),
        };

        if (card.Status == CardStatus.Failed)
        {
            body.Children.Add(new TextBlock
            {
                Text = Localization.T(card.ErrorTitleKey ?? "popup.error.translationFailed"),
                FontFamily = PopupFont(),
                Foreground = danger,
                FontSize = PopupFontSize("PopupFontSizeCaption", 12),
                FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                Margin = new Thickness(0, 0, 0, 4),
            });
            if (!string.IsNullOrEmpty(card.ErrorMessage))
            {
                body.Children.Add(MakeBodyTextBlock(card.ErrorMessage, fg2));
            }
        }
        else if (card.Status == CardStatus.Cancelled)
        {
            body.Children.Add(new TextBlock
            {
                Text = Localization.T(card.ErrorTitleKey ?? "popup.status.cancelled"),
                FontFamily = PopupFont(),
                FontSize = PopupFontSize("PopupFontSizeCaption", 12),
                Foreground = fg3,
                Margin = new Thickness(0, 0, 0, 4),
            });
            if (!string.IsNullOrEmpty(textSnapshot))
            {
                textClip = MakeClippedResultText(textSnapshot, fg, card.Expanded, showStreamCursor: false, accent);
                body.Children.Add(textClip);
            }
        }
        else if (card.Status == CardStatus.Translating && string.IsNullOrEmpty(textSnapshot))
        {
            // 空流式：闪烁光标占位（对齐 .stream-cursor）
            var cursor = MakeStreamCursor(accent);
            body.Children.Add(cursor);
        }
        else if (card.Status == CardStatus.Pending)
        {
            body.Children.Add(new TextBlock
            {
                Text = "…",
                FontFamily = PopupFont(),
                FontSize = PopupFontSize("PopupFontSizeBody", 13),
                Foreground = fg3,
                Opacity = 0.55,
            });
        }
        else
        {
            var streaming = card.Status == CardStatus.Translating;
            textClip = MakeClippedResultText(textSnapshot, fg, card.Expanded, streaming, accent);
            body.Children.Add(textClip);
        }

        // 溢出：≈ 4 行 (6.4em @ 13px) → 展开全文
        var overflow = EstimateHasOverflow(textSnapshot);
        card.HasOverflow = overflow;
        var canExpand = overflow && (
            card.Status is CardStatus.Finished or CardStatus.Cancelled
            || (card.Status == CardStatus.Translating && !string.IsNullOrEmpty(textSnapshot)));
        if (canExpand)
        {
            var expandLabelTb = new TextBlock
            {
                Text = card.Expanded
                    ? Localization.T("popup.action.collapseFull")
                    : Localization.T("popup.action.expandFull"),
                FontFamily = PopupFont(),
                FontSize = PopupFontSize("PopupFontSizeBadge", 11),
                Foreground = fg2,
                VerticalAlignment = VerticalAlignment.Center,
            };
            expandChevron = new FontIcon
            {
                Glyph = "\uE70D",
                FontSize = PopupFontSize("PopupFontSizeMeta", 10),
                Foreground = fg2,
                VerticalAlignment = VerticalAlignment.Center,
                RenderTransformOrigin = new Windows.Foundation.Point(0.5, 0.5),
                RenderTransform = new RotateTransform { Angle = card.Expanded ? 180 : 0 },
            };
            var expandBtn = new Button
            {
                Background = new SolidColorBrush(Colors.Transparent),
                BorderThickness = new Thickness(0),
                Padding = new Thickness(4, 2, 4, 2),
                Margin = new Thickness(-2, 4, 0, 0),
                HorizontalAlignment = HorizontalAlignment.Left,
                Content = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 3,
                    Children = { expandLabelTb, expandChevron },
                },
            };
            var clipRef = textClip;
            var chevronRef = expandChevron;
            expandBtn.Click += (_, _) =>
            {
                if (!_bridge.State.Cards.TryGetValue(serviceId, out var live) || clipRef is null)
                    return;
                var expanding = !live.Expanded;
                _bridge.ToggleCardExpanded(serviceId, raiseUi: false);
                expandLabelTb.Text = expanding
                    ? Localization.T("popup.action.collapseFull")
                    : Localization.T("popup.action.expandFull");
                AnimateTextClipExpand(clipRef, chevronRef, expanding);
            };
            body.Children.Add(expandBtn);
        }

        // actions: margin-top 6；左 22px 图标；右 model 10px + tokens
        var showMeta = card.Protocol != "microsoft_edge"
            && (!string.IsNullOrWhiteSpace(card.ModelName) || card.Usage is not null);
        var canActOnText = !string.IsNullOrEmpty(textSnapshot) || card.Status is CardStatus.Finished;
        var showRetry = card.Status is CardStatus.Failed or CardStatus.Cancelled;

        if (canActOnText || showRetry || showMeta)
        {
            var actions = new Grid { Margin = new Thickness(0, 6, 0, 0) };
            actions.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            actions.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

            var left = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 3,
                VerticalAlignment = VerticalAlignment.Center,
            };

            if (canActOnText)
            {
                var speakBtn = MakeIconButton("\uE767", Localization.T("popup.tooltip.speak"), 22, 12);
                speakBtn.Click += async (_, _) =>
                {
                    try { await _speech.SpeakAsync(textSnapshot); }
                    catch { /* ignore */ }
                };
                left.Children.Add(speakBtn);

                var copyBtn = MakeIconButton("\uE8C8", Localization.T("popup.tooltip.copy"), 22, 12);
                copyBtn.Click += (_, _) =>
                {
                    CopyText(textSnapshot);
                    ShowTipBar(Localization.T("popup.toast.copied"));
                };
                left.Children.Add(copyBtn);
            }

            if (showRetry)
            {
                var retryBtn = MakeIconButton("\uE72C", Localization.T("popup.tooltip.retry"), 22, 12);
                retryBtn.Click += (_, _) => _bridge.RetryTranslation();
                left.Children.Add(retryBtn);
            }

            Grid.SetColumn(left, 0);
            actions.Children.Add(left);

            if (showMeta)
            {
                var right = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 7,
                    VerticalAlignment = VerticalAlignment.Center,
                    HorizontalAlignment = HorizontalAlignment.Right,
                };

                if (!string.IsNullOrWhiteSpace(card.ModelName))
                {
                    right.Children.Add(new TextBlock
                    {
                        Text = card.ModelName,
                        FontSize = PopupFontSize("PopupFontSizeMeta", 10), // 0.625rem
                        FontFamily = PopupMonoFont(),
                        Foreground = fg3,
                        VerticalAlignment = VerticalAlignment.Center,
                        MaxWidth = 150,
                        TextTrimming = TextTrimming.CharacterEllipsis,
                    });
                }

                if (card.Usage is not null)
                {
                    var tokens = new StackPanel
                    {
                        Orientation = Orientation.Horizontal,
                        Spacing = 7,
                        VerticalAlignment = VerticalAlignment.Center,
                    };
                    tokens.Children.Add(MakeTokenChip("↑", card.Usage.InputTokens, fg3));
                    tokens.Children.Add(new Border
                    {
                        Width = 1,
                        Height = 9,
                        Background = borderBrush,
                        VerticalAlignment = VerticalAlignment.Center,
                        Opacity = 0.9,
                    });
                    tokens.Children.Add(MakeTokenChip("↓", card.Usage.OutputTokens, fg3));
                    right.Children.Add(tokens);
                }

                Grid.SetColumn(right, 1);
                actions.Children.Add(right);
            }

            body.Children.Add(actions);
        }

        return body;
    }

    /// <summary>正文 clip 容器：未展开 MaxHeight≈4 行，展开时测高动画（对齐 .result-text-clip）。</summary>
    private Border MakeClippedResultText(string text, Brush fg, bool expanded, bool showStreamCursor, Brush accent)
    {
        // 避免 InlineUIContainer + Forever Storyboard（流式高频 Rebuild 时易崩）
        UIElement bodyEl;
        if (showStreamCursor)
        {
            var tb = MakeBodyTextBlock(text, fg);
            var cursor = MakeStreamCursor(accent);
            var stack = new StackPanel { Spacing = 0 };
            stack.Children.Add(tb);
            var cursorRow = new StackPanel { Orientation = Orientation.Horizontal };
            cursorRow.Children.Add(cursor);
            stack.Children.Add(cursorRow);
            bodyEl = stack;
        }
        else
        {
            bodyEl = MakeBodyTextBlock(text, fg);
        }

        var overflow = EstimateHasOverflow(text);
        var clip = new Border
        {
            Child = bodyEl,
            Tag = EstimateFullTextHeight(text),
        };

        if (overflow && !expanded)
            clip.MaxHeight = ResultClipCollapsedPx;
        else if (overflow && expanded)
            clip.MaxHeight = EstimateFullTextHeight(text);

        return clip;
    }

    /// <summary>流式光标：1px 竖线 + 1s 闪烁（对齐 .stream-cursor / blink）。</summary>
    private Border MakeStreamCursor(Brush accent)
    {
        var bodyPx = PopupFontSize("PopupFontSizeBody", 13);
        var cursor = new Border
        {
            Width = 1,
            Height = bodyPx + 1,
            Background = accent,
            Margin = new Thickness(1, 0, 0, 0),
            VerticalAlignment = VerticalAlignment.Bottom,
            Opacity = 1,
        };
        StartBlinkForever(cursor);
        return cursor;
    }

    /// <summary>原文/译文共用：Segoe UI Variable + 13px + line-height ~1.6。</summary>
    private TextBlock MakeBodyTextBlock(string text, Brush fg)
    {
        var size = PopupFontSize("PopupFontSizeBody", 13);
        var line = PopupFontSize("PopupLineHeightBody", size * 1.6);
        return new TextBlock
        {
            Text = text,
            FontFamily = PopupFont(),
            FontSize = size,
            Foreground = fg,
            TextWrapping = TextWrapping.WrapWholeWords,
            IsTextSelectionEnabled = true,
            LineHeight = line,
        };
    }

    private static FontFamily PopupFont() =>
        TryResourceFont("PopupFontFamily")
        ?? new FontFamily("Segoe UI Variable Text, Segoe UI, Microsoft YaHei UI");

    private static FontFamily PopupMonoFont() =>
        TryResourceFont("PopupMonoFontFamily")
        ?? new FontFamily("Cascadia Mono, Consolas, Courier New");

    private static double PopupFontSize(string key, double fallback)
    {
        try
        {
            if (Application.Current?.Resources.TryGetValue(key, out var v) == true)
            {
                if (v is double d)
                    return d;
                if (v is float f)
                    return f;
                if (v is int i)
                    return i;
            }
        }
        catch
        {
            // ignore
        }

        return fallback;
    }

    private static FontFamily? TryResourceFont(string key)
    {
        try
        {
            if (Application.Current?.Resources.TryGetValue(key, out var v) == true && v is FontFamily ff)
                return ff;
        }
        catch
        {
            // ignore
        }

        return null;
    }

    private static double EstimateFullTextHeight(string text)
    {
        if (string.IsNullOrEmpty(text))
            return ResultClipCollapsedPx;
        var lines = text.Replace("\r\n", "\n").Split('\n');
        var softLines = 0;
        foreach (var line in lines)
            softLines += Math.Max(1, (int)Math.Ceiling(line.Length / 32.0));
        var hardLines = lines.Length;
        var lineCount = Math.Max(hardLines, softLines);
        var lineH = PopupFontSize("PopupLineHeightBody", 20.8);
        return Math.Max(ResultClipCollapsedPx, lineCount * lineH);
    }

    private static bool EstimateHasOverflow(string? text)
    {
        if (string.IsNullOrEmpty(text))
            return false;
        var lines = text.Replace("\r\n", "\n").Split('\n');
        var hardLines = lines.Length;
        // 约 32 汉字/行（卡宽 ~440 - pad）
        var softLines = 0;
        foreach (var line in lines)
            softLines += Math.Max(1, (int)Math.Ceiling(line.Length / 32.0));
        return Math.Max(hardLines, softLines) > 4;
    }

    private static bool IsDescendantOf(DependencyObject? node, DependencyObject ancestor)
    {
        while (node is not null)
        {
            if (ReferenceEquals(node, ancestor))
                return true;
            node = VisualTreeHelper.GetParent(node);
        }
        return false;
    }

    /// <summary>原型 .result-tokens .tok：↑/↓ + 数字（0.625rem）。</summary>
    private static UIElement MakeTokenChip(string arrow, int value, Brush fg3)
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var meta = PopupFontSize("PopupFontSizeMeta", 10);
        row.Children.Add(new TextBlock
        {
            Text = arrow,
            FontFamily = PopupFont(),
            FontSize = meta,
            Foreground = fg3,
            Opacity = 0.55,
            VerticalAlignment = VerticalAlignment.Center,
        });
        row.Children.Add(new TextBlock
        {
            Text = value.ToString(),
            FontSize = meta,
            FontFamily = PopupMonoFont(),
            Foreground = fg3,
            VerticalAlignment = VerticalAlignment.Center,
        });
        return row;
    }

    private static Button MakeIconButton(string glyph, string tip, double size = 28, double fontSize = 12)
    {
        var fg2 = TryResourceBrushStatic("PopupFg2Brush")
            ?? new SolidColorBrush(Color.FromArgb(255, 0x5D, 0x5D, 0x5D));
        var content = new FontIcon
        {
            Glyph = glyph,
            FontSize = fontSize,
            Foreground = fg2,
        };

        var btn = new Button
        {
            Width = size,
            Height = size,
            Padding = new Thickness(0),
            Content = content,
            Background = new SolidColorBrush(Colors.Transparent),
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(4),
        };

        if (Application.Current?.Resources.TryGetValue("PopupMetaButtonStyle", out var styleObj) == true
            && styleObj is Style style)
        {
            btn.Style = style;
            btn.Width = size;
            btn.Height = size;
            btn.Content = content;
        }

        ToolTipService.SetToolTip(btn, tip);
        return btn;
    }

    private void UpdatePinButtonVisual()
    {
        var pinTip = Localization.T(_alwaysOnTop ? "popup.tooltip.unpin" : "popup.tooltip.pin");
        if (!string.IsNullOrEmpty(_backdropMode) && _backdropMode != "unset")
            pinTip = $"{pinTip} · {_backdropMode}";
        ToolTipService.SetToolTip(PinButton, pinTip);
        var accent = TryResourceBrush("PopupAccentBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0xD5, 0x5A, 0x1F));
        var soft = TryResourceBrush("PopupAccentSoftBrush")
            ?? new SolidColorBrush(Color.FromArgb(0x1A, 0xD5, 0x5A, 0x1F));
        var fg = TryResourceBrush("PopupFgBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0x1A, 0x1A, 0x1A));

        if (PinIcon is not null)
        {
            PinIcon.Glyph = _alwaysOnTop ? "\uE841" : "\uE840";
            PinIcon.Foreground = _alwaysOnTop ? accent : fg;
        }

        PinButton.Background = _alwaysOnTop ? soft : new SolidColorBrush(Colors.Transparent);
    }

    private static Brush? TryResourceBrush(string key) => TryResourceBrushStatic(key);

    private static Brush? TryResourceBrushStatic(string key)
    {
        try
        {
            if (Application.Current?.Resources is null)
                return null;
            if (Application.Current.Resources.TryGetValue(key, out var v) && v is Brush b)
                return b;
        }
        catch
        {
            // ignore
        }

        return null;
    }

    private static Brush? TryThemeBrush(string key) => TryResourceBrushStatic(key);

    private void UpdateCharCount()
    {
        var n = SourceTextBox.Text?.Length ?? 0;
        CharCountText.Text = Localization.T("popup.charCount", "count", n.ToString());
    }

    private void ShowTipBar(string message)
    {
        TipBar.Message = message;
        TipBar.IsOpen = true;
        _tipTimer?.Stop();
        _tipTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2.5) };
        _tipTimer.Tick += (_, _) =>
        {
            TipBar.IsOpen = false;
            _tipTimer?.Stop();
        };
        _tipTimer.Start();
    }

    /// <param name="notifyHost">
    /// 为 false 时只本地 Resize（折叠动画逐帧跟窗用），避免每帧 IPC set_size 往返造成顿挫。
    /// </param>
    /// <param name="liveFollow">动画跟随时用更小阈值 + 优先 ActualHeight。</param>
    private void ReportContentSize(bool notifyHost = true, bool liveFollow = false)
    {
        if (_sizeReportBusy || !IsPopupVisible)
            return;
        _sizeReportBusy = true;
        try
        {
            double contentH;
            if (liveFollow && ContentPanel.ActualHeight > 1)
            {
                // 折叠 MaxHeight 动画每帧会改 ActualHeight；直接读避免 Measure 与动画态打架
                contentH = ContentPanel.ActualHeight;
            }
            else
            {
                var contentWidth = PopupLogicalWidth - 28;
                ContentPanel.Measure(new Windows.Foundation.Size(contentWidth, double.PositiveInfinity));
                contentH = ContentPanel.DesiredSize.Height;
            }

            // 防止 NaN/Infinity 经 Clamp 后仍污染 Resize → 1px「闪一下消失」
            if (double.IsNaN(contentH) || double.IsInfinity(contentH) || contentH < 0)
                contentH = 120;
            var h = Math.Clamp(contentH + 44 + 40 + 16, 160, 720);
            const double w = PopupLogicalWidth;

            var threshold = liveFollow ? 0.5 : 2.0;
            if (Math.Abs(h - _lastReportedHeight) < threshold)
                return;

            _lastReportedHeight = h;
            TryResizeLogical(w, h);
            if (notifyHost)
                _bridge.ReportContentSize(w, h);
        }
        catch
        {
            // ignore
        }
        finally
        {
            _sizeReportBusy = false;
        }
    }

    private void TryResizeLogical(double width, double height)
    {
        if (double.IsNaN(width) || double.IsNaN(height)
            || double.IsInfinity(width) || double.IsInfinity(height)
            || width < 1 || height < 1)
        {
            return;
        }

        var scale = Content?.XamlRoot?.RasterizationScale ?? GetDpiScaleFallback();
        if (scale < 0.5 || double.IsNaN(scale) || double.IsInfinity(scale))
            scale = 1.0;

        var w = Math.Max(160, (int)Math.Round(width * scale));
        var h = Math.Max(120, (int)Math.Round(height * scale));
        // 物理像素上限，避免异常量测把窗口撑到不可用
        w = Math.Min(w, 2400);
        h = Math.Min(h, 2000);
        _appWindow.Resize(new SizeInt32(w, h));
    }

    private static double GetDpiScaleFallback() => 1.0;

    private static void CopyText(string? text)
    {
        if (string.IsNullOrEmpty(text))
            return;
        try
        {
            var dp = new DataPackage();
            dp.SetText(text);
            Clipboard.SetContent(dp);
        }
        catch
        {
            // ignore
        }
    }

    // ——— 事件处理 ———

    private void RootGrid_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        // 防抖：布局抖动时合并为一次 report
        if (!IsPopupVisible)
            return;
        _sizeDebounce?.Stop();
        _sizeDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(80) };
        _sizeDebounce.Tick += (_, _) =>
        {
            _sizeDebounce?.Stop();
            ReportContentSize();
        };
        _sizeDebounce.Start();
    }

    private void PinButton_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            SetAlwaysOnTop(!_alwaysOnTop);
            ShowTipBar(Localization.T(_alwaysOnTop ? "popup.toast.pinned" : "popup.toast.unpinned"));
        }
        catch
        {
            ShowTipBar(Localization.T("popup.error.pinFailed"));
        }
    }

    private void OcrButton_Click(object sender, RoutedEventArgs e) =>
        _bridge.TriggerOcrTranslation();

    private void SettingsButton_Click(object sender, RoutedEventArgs e) =>
        _bridge.OpenSettings();

    private void MinButton_Click(object sender, RoutedEventArgs e) => HidePopup();

    private void CloseButton_Click(object sender, RoutedEventArgs e) => HidePopup();

    private void FavButton_Click(object sender, RoutedEventArgs e) =>
        ShowTipBar(Localization.T("popup.toast.featureWip"));

    private void BookmarkButton_Click(object sender, RoutedEventArgs e) =>
        ShowTipBar(Localization.T("popup.toast.featureWip"));

    private void ThemeButton_Click(object sender, RoutedEventArgs e) =>
        ShowTipBar(Localization.T("popup.toast.featureWip"));

    private void CloseButton_PointerEntered(object sender, PointerRoutedEventArgs e)
    {
        CloseButton.Background = TryResourceBrush("PopupDestructiveBrush")
            ?? new SolidColorBrush(Color.FromArgb(255, 0xC4, 0x2B, 0x1C));
        if (CloseIcon is not null)
            CloseIcon.Foreground = new SolidColorBrush(Colors.White);
    }

    private void CloseButton_PointerExited(object sender, PointerRoutedEventArgs e)
    {
        CloseButton.Background = new SolidColorBrush(Colors.Transparent);
        if (CloseIcon is not null)
        {
            CloseIcon.Foreground = TryResourceBrush("PopupFgBrush")
                ?? new SolidColorBrush(Color.FromArgb(255, 0x1A, 0x1A, 0x1A));
        }
    }

    private void SourceCard_GotFocus(object sender, RoutedEventArgs e) => SetSourceCardFocused(true);

    private void SourceCard_LostFocus(object sender, RoutedEventArgs e) =>
        SetSourceCardFocused(SourceTextBox.FocusState != FocusState.Unfocused);

    private void SourceTextBox_GotFocus(object sender, RoutedEventArgs e) => SetSourceCardFocused(true);

    private void SourceTextBox_LostFocus(object sender, RoutedEventArgs e) => SetSourceCardFocused(false);

    private void SetSourceCardFocused(bool focused)
    {
        if (SourceCardBorder is null)
            return;
        if (focused)
        {
            SourceCardBorder.BorderBrush = TryResourceBrush("PopupAccentBrush")
                ?? new SolidColorBrush(Color.FromArgb(255, 0xD5, 0x5A, 0x1F));
            SourceCardBorder.BorderThickness = new Thickness(1.5);
        }
        else
        {
            SourceCardBorder.BorderBrush = TryResourceBrush("PopupBorderBrush")
                ?? new SolidColorBrush(Color.FromArgb(0x0F, 0, 0, 0));
            SourceCardBorder.BorderThickness = new Thickness(1);
        }
    }

    private void SourceLangButton_Click(object sender, RoutedEventArgs e)
    {
        // Flyout 已绑在 Button.Flyout，点击自动展开
    }

    private void TargetLangButton_Click(object sender, RoutedEventArgs e)
    {
    }

    private void SourceLangList_ItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not TranslationLanguage lang)
            return;
        SourceLangFlyout.Hide();
        _bridge.SetSessionLanguages(lang.Code, _bridge.SessionTargetLang);
        RefreshLanguageLabels();
    }

    private void TargetLangList_ItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not TranslationLanguage lang)
            return;
        TargetLangFlyout.Hide();
        _bridge.SetSessionLanguages(_bridge.SessionSourceLang, lang.Code);
        RefreshLanguageLabels();
    }

    private void SourceCopy_Click(object sender, RoutedEventArgs e)
    {
        CopyText(SourceTextBox.Text);
        ShowTipBar(Localization.T("popup.toast.copied"));
    }

    private async void SourceSpeak_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            await _speech.SpeakAsync(SourceTextBox.Text ?? "");
        }
        catch
        {
            // ignore
        }
    }

    private void SourceTextBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        if (_suppressSourceTextEvents)
            return;
        _bridge.NotifySourceEdited(SourceTextBox.Text ?? "");
        UpdateCharCount();
    }

    /// <summary>
    /// 必须用 PreviewKeyDown：TextBox AcceptsReturn 时 KeyDown 已太晚，换行会先插入。
    /// 对齐 Vue SourceCard：Enter 提交，Shift+Enter 换行。
    /// </summary>
    private void SourceTextBox_PreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Enter)
            return;

        var shift = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Shift);
        if (shift.HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down))
            return;

        e.Handled = true;
        _bridge.StartTranslation(SourceTextBox.Text);
    }

    // ——— 动画：loading / 折叠 / 展开全文（对齐 components.css 0.15s）———

    /// <summary>状态点 pulse（1.2s ease-in-out infinite，对齐 .result-header-dot / .status-dot.loading）。</summary>
    private void StartPulseForever(UIElement target)
    {
        var anim = new DoubleAnimation
        {
            From = 1.0,
            To = 0.4,
            Duration = TimeSpan.FromMilliseconds(600),
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = new SineEase { EasingMode = EasingMode.EaseInOut },
        };
        Storyboard.SetTarget(anim, target);
        Storyboard.SetTargetProperty(anim, "Opacity");
        var sb = new Storyboard();
        sb.Children.Add(anim);
        TrackCardStoryboard(sb);
        sb.Begin();
    }

    /// <summary>流式光标闪烁（1s steps，近似 blink）。</summary>
    private void StartBlinkForever(UIElement target)
    {
        var anim = new DoubleAnimationUsingKeyFrames
        {
            RepeatBehavior = RepeatBehavior.Forever,
            Duration = TimeSpan.FromSeconds(1),
        };
        anim.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = KeyTime.FromTimeSpan(TimeSpan.Zero), Value = 1 });
        anim.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = KeyTime.FromTimeSpan(TimeSpan.FromMilliseconds(500)), Value = 0 });
        Storyboard.SetTarget(anim, target);
        Storyboard.SetTargetProperty(anim, "Opacity");
        var sb = new Storyboard();
        sb.Children.Add(anim);
        TrackCardStoryboard(sb);
        sb.Begin();
    }

    /// <summary>可停用的 pulse（状态栏圆点）；loading 切换时复用同一 Storyboard 引用。</summary>
    private static void SetLoadingPulse(UIElement target, ref Storyboard? sb, bool loading)
    {
        sb?.Stop();
        sb = null;
        target.Opacity = 1;
        if (!loading)
            return;

        var anim = new DoubleAnimation
        {
            From = 1.0,
            To = 0.4,
            Duration = TimeSpan.FromMilliseconds(500),
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = new SineEase { EasingMode = EasingMode.EaseInOut },
        };
        Storyboard.SetTarget(anim, target);
        Storyboard.SetTargetProperty(anim, "Opacity");
        sb = new Storyboard();
        sb.Children.Add(anim);
        sb.Begin();
    }

    /// <summary>整卡 body 折叠/展开：MaxHeight + Opacity + chevron 旋转（~150ms）。</summary>
    private void AnimateCardBodyCollapse(Border bodyHost, FrameworkElement body, FontIcon chevron, bool collapsing)
    {
        // 先量测展开高度
        bodyHost.Clip = null;
        body.Measure(new Windows.Foundation.Size(bodyHost.ActualWidth > 0 ? bodyHost.ActualWidth : PopupLogicalWidth - 28, double.PositiveInfinity));
        var fullH = Math.Max(body.DesiredSize.Height, body.ActualHeight);
        if (fullH < 1)
            fullH = 1;

        double fromH;
        double toH;
        double fromOp;
        double toOp;
        double fromAngle;
        double toAngle;

        if (collapsing)
        {
            fromH = bodyHost.ActualHeight > 1 ? bodyHost.ActualHeight : fullH;
            toH = 0;
            fromOp = 1;
            toOp = 0;
            fromAngle = 0;
            toAngle = -90;
            bodyHost.MaxHeight = fromH;
            bodyHost.Opacity = 1;
        }
        else
        {
            fromH = 0;
            toH = fullH;
            fromOp = 0;
            toOp = 1;
            fromAngle = -90;
            toAngle = 0;
            bodyHost.MaxHeight = 0;
            bodyHost.Opacity = 0;
        }

        var dur = TimeSpan.FromMilliseconds(CardAnimMs);
        var ease = new QuadraticEase { EasingMode = EasingMode.EaseInOut };

        var hAnim = new DoubleAnimation
        {
            From = fromH,
            To = toH,
            Duration = dur,
            EasingFunction = ease,
            // MaxHeight 影响布局：WinUI 默认禁用 dependent animation，不设则动画被静默跳过
            EnableDependentAnimation = true,
        };
        Storyboard.SetTarget(hAnim, bodyHost);
        Storyboard.SetTargetProperty(hAnim, "MaxHeight");

        var oAnim = new DoubleAnimation
        {
            From = fromOp,
            To = toOp,
            Duration = TimeSpan.FromMilliseconds(120),
            EasingFunction = ease,
        };
        Storyboard.SetTarget(oAnim, bodyHost);
        Storyboard.SetTargetProperty(oAnim, "Opacity");

        if (chevron.RenderTransform is not RotateTransform rot)
        {
            rot = new RotateTransform { Angle = fromAngle };
            chevron.RenderTransform = rot;
            chevron.RenderTransformOrigin = new Windows.Foundation.Point(0.5, 0.5);
        }
        else
        {
            rot.Angle = fromAngle;
        }

        var rAnim = new DoubleAnimation
        {
            From = fromAngle,
            To = toAngle,
            Duration = dur,
            EasingFunction = ease,
        };
        Storyboard.SetTarget(rAnim, rot);
        Storyboard.SetTargetProperty(rAnim, "Angle");

        var sb = new Storyboard();
        sb.Children.Add(hAnim);
        sb.Children.Add(oAnim);
        sb.Children.Add(rAnim);
        sb.Completed += (_, _) =>
        {
            if (collapsing)
            {
                bodyHost.MaxHeight = 0;
                bodyHost.Opacity = 0;
                bodyHost.Clip = new RectangleGeometry { Rect = new Windows.Foundation.Rect(0, 0, 0, 0) };
            }
            else
            {
                bodyHost.MaxHeight = Math.Max(fullH * 2, 8000);
                bodyHost.Opacity = 1;
                bodyHost.Clip = null;
            }

            // 终帧再通知宿主（动画期间只本地跟窗）
            FollowWindowSizeDuringAnim(0);
        };
        sb.Begin();

        // 与卡片 150ms 动画同帧跟窗高（对齐 Vue ResizeObserver + rAF）
        FollowWindowSizeDuringAnim(CardAnimMs);
    }

    /// <summary>展开全文 / 收起：clip MaxHeight 插值 + chevron 180°。</summary>
    private void AnimateTextClipExpand(Border textClip, FontIcon? chevron, bool expanding)
    {
        var fullH = textClip.Tag is double d && d > 0
            ? d
            : EstimateFullTextHeight(ExtractTextBlockText(textClip.Child as TextBlock));
        // 优先用实测
        if (textClip.Child is FrameworkElement fe)
        {
            fe.Measure(new Windows.Foundation.Size(
                textClip.ActualWidth > 0 ? textClip.ActualWidth : PopupLogicalWidth - 52,
                double.PositiveInfinity));
            if (fe.DesiredSize.Height > ResultClipCollapsedPx)
                fullH = fe.DesiredSize.Height;
        }

        var fromH = expanding
            ? (textClip.ActualHeight > 0 ? textClip.ActualHeight : ResultClipCollapsedPx)
            : (textClip.ActualHeight > 0 ? textClip.ActualHeight : fullH);
        var toH = expanding ? fullH : ResultClipCollapsedPx;

        textClip.MaxHeight = fromH;

        var hAnim = new DoubleAnimation
        {
            From = fromH,
            To = toH,
            Duration = TimeSpan.FromMilliseconds(CardAnimMs),
            EasingFunction = new QuadraticEase { EasingMode = EasingMode.EaseInOut },
            EnableDependentAnimation = true,
        };
        Storyboard.SetTarget(hAnim, textClip);
        Storyboard.SetTargetProperty(hAnim, "MaxHeight");

        var sb = new Storyboard();
        sb.Children.Add(hAnim);

        if (chevron is not null)
        {
            if (chevron.RenderTransform is not RotateTransform rot)
            {
                rot = new RotateTransform();
                chevron.RenderTransform = rot;
                chevron.RenderTransformOrigin = new Windows.Foundation.Point(0.5, 0.5);
            }

            var rAnim = new DoubleAnimation
            {
                From = expanding ? 0 : 180,
                To = expanding ? 180 : 0,
                Duration = TimeSpan.FromMilliseconds(CardAnimMs),
                EasingFunction = new QuadraticEase { EasingMode = EasingMode.EaseInOut },
            };
            Storyboard.SetTarget(rAnim, rot);
            Storyboard.SetTargetProperty(rAnim, "Angle");
            sb.Children.Add(rAnim);
        }

        sb.Completed += (_, _) =>
        {
            textClip.MaxHeight = expanding ? fullH : ResultClipCollapsedPx;
            textClip.Tag = fullH;
            FollowWindowSizeDuringAnim(0);
        };
        sb.Begin();
        FollowWindowSizeDuringAnim(CardAnimMs);
    }

    private static string ExtractTextBlockText(TextBlock? tb)
    {
        if (tb is null)
            return "";
        if (!string.IsNullOrEmpty(tb.Text))
            return tb.Text;
        // 使用 Inlines 时 Text 可能为空
        var sb = new System.Text.StringBuilder();
        foreach (var inline in tb.Inlines)
        {
            if (inline is Run run)
                sb.Append(run.Text);
        }
        return sb.ToString();
    }

    /// <summary>
    /// 卡片折叠/展开全文期间 ~60fps 本地跟窗高；结束再 report 宿主。
    /// durationMs=0 表示只做终帧同步（含 notifyHost）。
    /// </summary>
    private void FollowWindowSizeDuringAnim(int durationMs)
    {
        _sizeDebounce?.Stop();
        _sizeFollowTimer?.Stop();

        if (durationMs <= 0)
        {
            ReportContentSize(notifyHost: true, liveFollow: false);
            return;
        }

        const int frameMs = 16;
        _sizeFollowRemaining = Math.Max(2, durationMs / frameMs + 3);
        _sizeFollowTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(frameMs) };
        _sizeFollowTimer.Tick += (_, _) =>
        {
            ReportContentSize(notifyHost: false, liveFollow: true);
            if (--_sizeFollowRemaining > 0)
                return;
            _sizeFollowTimer?.Stop();
            _sizeFollowTimer = null;
            ReportContentSize(notifyHost: true, liveFollow: false);
        };
        _sizeFollowTimer.Start();
        ReportContentSize(notifyHost: false, liveFollow: true);
    }

    private void ScheduleReportContentSize()
    {
        // 动画跟窗进行中勿被 SizeChanged 防抖打断
        if (_sizeFollowTimer is { IsEnabled: true })
            return;

        _sizeDebounce?.Stop();
        _sizeDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(40) };
        _sizeDebounce.Tick += (_, _) =>
        {
            _sizeDebounce?.Stop();
            ReportContentSize();
        };
        _sizeDebounce.Start();
    }

    private void SwapLang_Click(object sender, RoutedEventArgs e)
    {
        if (!_bridge.TrySwapLanguages())
        {
            if (!string.IsNullOrEmpty(_bridge.TipMessage))
            {
                ShowTipBar(_bridge.TipMessage);
                _bridge.ClearTip();
            }
        }
        else
        {
            RefreshLanguageLabels();
        }
    }

    private void StatusAction_Click(object sender, RoutedEventArgs e)
    {
        switch (_bridge.StatusAction)
        {
            case StatusActionKind.Cancel:
                _bridge.CancelTranslation();
                break;
            case StatusActionKind.Retry:
                _bridge.RetryTranslation();
                break;
        }
    }

    private void TipBar_CloseButtonClick(InfoBar sender, object args)
    {
        TipBar.IsOpen = false;
    }
}
