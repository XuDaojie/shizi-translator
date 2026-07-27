using System.Numerics;
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
    /// <summary>禁用 DWM 对 HWND 尺寸变化的过渡动画，避免与卡片跟窗逐帧 Resize 叠成「弹一下再缩」。</summary>
    private const int DwmwaTransitionsForcedisabled = 3;

    private readonly AppWindow _appWindow;
    private readonly BridgeService _bridge = BridgeService.Instance;
    private readonly SpeechService _speech = new();
    private bool _alwaysOnTop;
    private bool _suppressSourceTextEvents;
    private bool _startupStarted;
    private bool _sizeReportBusy;
    private double _lastReportedHeight;
    private int _lastResizePhysicalH;
    private DispatcherTimer? _tipTimer;
    private DispatcherTimer? _sizeDebounce;
    /// <summary>卡片伸缩动画期间：屏蔽 SizeChanged，每帧按内容真实期望高跟窗（对齐 Vue ResizeObserver）。</summary>
    private bool _sizeFollowActive;
    /// <summary>并行 body 折叠/展开动画数；全部完成后再停跟窗并 notifyHost。</summary>
    private int _pendingBodySizeAnims;
    private EventHandler<object>? _sizeFollowRenderHandler;
    private Storyboard? _statusDotPulse;
    private SystemBackdropConfiguration? _micaConfig;
    /// <summary>卡片 loading 脉冲/光标闪烁等 Forever Storyboard；Rebuild 前必须 Stop，否则目标被拆掉会崩进程。</summary>
    private readonly List<Storyboard> _cardStoryboards = new();
    /// <summary>结果卡视觉树缓存：结构未变时原地补丁文本/折叠，避免 delta 全量重建掐掉自动展开动画。</summary>
    private readonly Dictionary<string, ResultCardVisual> _cardVisuals = new();
    private List<string> _cardVisualOrder = new();
    /// <summary>合并高频 RaiseUi（delta 流）为单次 UI 刷新，降低 Clear/Storyboard 抖动。</summary>
    private bool _uiRefreshQueued;

    /// <summary>单张结果卡的可补丁视觉引用。</summary>
    private sealed class ResultCardVisual
    {
        public required string Id { get; init; }
        public required Border Root { get; init; }
        public required Border BodyHost { get; init; }
        public required FrameworkElement Body { get; init; }
        public required FontIcon CollapseChevron { get; init; }
        public required Button CollapseButton { get; init; }
        public Border? TextClip { get; set; }
        public TextBlock? ResultText { get; set; }
        public bool Collapsed { get; set; }
        public string StructureKey { get; set; } = "";
    }

    /// <summary>与 Vue components.css 卡片折叠 / 展开全文 transition 对齐（0.15s）。</summary>
    private const int CardAnimMs = 150;
    /// <summary>正文折叠上限 6.4em @ 13px line-height 1.6 → 约 4 行。</summary>
    private const double ResultClipCollapsedPx = 20.8 * 4;
    /// <summary>
    /// 结果卡 body 展开态 MaxHeight 上限。必须用大有限值：ClearValue/Infinity 会让 Measure 偶发虚高，
    /// 窗高猛拉后再被 Actual 纠正（用户看到的「突然更长又缩回」）。
    /// </summary>
    private const double BodyExpandedMax = 8000;
    private const double PopupMinLogicalHeight = 160;
    private const double PopupMaxLogicalHeight = 720;
    /// <summary>共享 ThemeShadow（对齐原型 box-shadow 轻卡阴影）；Receivers 挂在壳层。</summary>
    private ThemeShadow? _cardThemeShadow;
    private bool _sourceCardFocused;
    private bool _sourceCardHovered;
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
        TryDisableDwmResizeTransitions(hwnd);
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
        // 首帧布局后挂卡阴影 Receivers（需可视树就绪）
        if (Content is FrameworkElement rootFe)
        {
            rootFe.Loaded += (_, _) =>
            {
                EnsureCardThemeShadow();
                ApplyCardElevation(SourceCardBorder, elevated: false);
                ApplyCardElevation(LangBarBorder, elevated: false);
            };
        }

        _bridge.UiChanged += OnBridgeUiChanged;
        _bridge.LocaleChanged += OnLocaleChanged;

        // 首帧后跑 ready / config / pending
        Activated += MainWindow_FirstActivated;
    }

    /// <summary>弹窗逻辑宽（对齐原型 468）。</summary>
    private const double PopupLogicalWidth = 468;

    /// <summary>
    /// 统一字族/字号：对齐 Open Design winui3（Segoe UI Variable + 13/12/11 阶梯）。
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

    /// <summary>
    /// 原型卡阴影：ThemeShadow + Translation.Z 近似
    /// <c>0 1.6px 3.2px / hover 0 2px 8px</c>；描边仍由 Border 承担 ring。
    /// </summary>
    private void EnsureCardThemeShadow()
    {
        if (_cardThemeShadow is not null)
            return;
        try
        {
            _cardThemeShadow = new ThemeShadow();
            // 阴影投射到壳与内容区背景（Receivers 必须在 elevated 元素之下）
            if (ShellBorder is not null)
                _cardThemeShadow.Receivers.Add(ShellBorder);
            if (RootGrid is not null && !ReferenceEquals(RootGrid, ShellBorder))
                _cardThemeShadow.Receivers.Add(RootGrid);
        }
        catch
        {
            _cardThemeShadow = null;
        }
    }

    private void ApplyCardElevation(UIElement? card, bool elevated)
    {
        if (card is null)
            return;
        EnsureCardThemeShadow();
        if (_cardThemeShadow is null)
            return;
        try
        {
            card.Shadow = _cardThemeShadow;
            var z = elevated
                ? PopupFontSize("PopupCardElevationHover", 28)
                : PopupFontSize("PopupCardElevation", 12);
            card.Translation = new Vector3(0, 0, (float)z);
        }
        catch
        {
            // ThemeShadow 在部分环境不可用时静默降级为纯描边
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
    /// 关掉 DWM 对窗口尺寸/显示的过渡。逐帧 Resize 时若保留系统动画，会与卡片 MaxHeight 动画
    /// 叠成不同步的「弹一下 / 回弹」（WebView 路径同样每帧 setSize，但 HWND 侧表现不同）。
    /// </summary>
    private static void TryDisableDwmResizeTransitions(IntPtr hwnd)
    {
        try
        {
            var disable = 1;
            _ = DwmSetWindowAttribute(hwnd, DwmwaTransitionsForcedisabled, ref disable, sizeof(int));
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
            // 全量重建/折叠后 ActualHeight 可能仍是旧值；强制 layout 再测高，避免清空原文后窗底留白
            try
            {
                ContentPanel.UpdateLayout();
                ShellBorder.UpdateLayout();
            }
            catch
            {
                // ignore
            }

            ReportContentSize(force: true);
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

        SyncResultCards();
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

    /// <summary>
    /// 同步结果卡：结构未变则原地补丁（文本 + 折叠动画）；否则全量重建。
    /// 对齐 Vue：collapsed class 切换走 CSS transition，而非销毁重建。
    /// </summary>
    private void SyncResultCards()
    {
        List<CardState> cards;
        try
        {
            var order = _bridge.State.CardOrder.ToList();
            cards = order
                .Select(id => _bridge.State.Cards.TryGetValue(id, out var c) ? c : null)
                .Where(c => c is not null)
                .Cast<CardState>()
                .ToList();
        }
        catch (Exception ex)
        {
            CrashLog.Write("SyncResultCards.Snapshot", ex);
            return;
        }

        if (TryPatchResultCards(cards))
            return;

        RebuildResultCardsFull(cards);
    }

    /// <summary>body 模板种类：决定能否原地补丁（不含 collapsed，折叠走动画）。</summary>
    private static string CardBodyKind(CardState card)
    {
        if (card.Status == CardStatus.Failed)
            return "failed";
        if (card.Status == CardStatus.Cancelled)
            return string.IsNullOrEmpty(card.Text) ? "cancelled-empty" : "cancelled-text";
        if (card.Status == CardStatus.Pending)
            return "pending";
        if (card.Status == CardStatus.Translating && string.IsNullOrEmpty(card.Text))
            return "stream-empty";
        // translating/finished 有正文 → 同一套 clip 模板；streaming 光标差异在补丁时处理
        return "text";
    }

    private static string CardStructureKey(CardState card) =>
        $"{card.ServiceInstanceId}\u001f{card.Status}\u001f{CardBodyKind(card)}\u001f{card.ShowActions}\u001f{card.Expanded}\u001f{card.ServiceName}\u001f{card.ServiceType}";

    private bool TryPatchResultCards(IReadOnlyList<CardState> cards)
    {
        if (_cardVisualOrder.Count != cards.Count || _cardVisuals.Count != cards.Count)
            return false;

        for (var i = 0; i < cards.Count; i++)
        {
            if (!string.Equals(_cardVisualOrder[i], cards[i].ServiceInstanceId, StringComparison.Ordinal))
                return false;
            if (!_cardVisuals.TryGetValue(cards[i].ServiceInstanceId, out var vis))
                return false;
            if (!string.Equals(vis.StructureKey, CardStructureKey(cards[i]), StringComparison.Ordinal))
                return false;
        }

        var needFollow = false;
        foreach (var card in cards)
        {
            var vis = _cardVisuals[card.ServiceInstanceId];
            // 正文增量
            if (vis.ResultText is not null && card.Text != vis.ResultText.Text)
            {
                vis.ResultText.Text = card.Text ?? "";
                if (vis.TextClip is not null)
                    vis.TextClip.Tag = EstimateFullTextHeight(card.Text ?? "");
                needFollow = true;
            }

            // 折叠态变化：自动展开/清空原文收起 → 走同一套 MaxHeight 动画
            if (vis.Collapsed != card.Collapsed)
            {
                ToolTipService.SetToolTip(
                    vis.CollapseButton,
                    Localization.T(card.Collapsed ? "popup.tooltip.expand" : "popup.tooltip.collapse"));
                AnimateCardBodyCollapse(vis.BodyHost, vis.Body, vis.CollapseChevron, collapsing: card.Collapsed);
                vis.Collapsed = card.Collapsed;
                needFollow = false; // 动画内已 StartContentSizeFollow
            }
        }

        if (needFollow)
        {
            try
            {
                ContentPanel.UpdateLayout();
            }
            catch
            {
                // ignore
            }

            ReportContentSize(force: true);
        }

        return true;
    }

    private void RebuildResultCardsFull(IReadOnlyList<CardState> cards)
    {
        // 翻译 delta 会极高频 Rebuild：先停 Forever 动画再 Clear，避免目标已出树仍被 Composition 驱动而崩进程
        StopCardStoryboards();
        ResultsPanel.Children.Clear();
        var prevCollapsed = _cardVisuals.ToDictionary(kv => kv.Key, kv => kv.Value.Collapsed);
        _cardVisuals.Clear();
        _cardVisualOrder = cards.Select(c => c.ServiceInstanceId).ToList();

        var pendingExpand = new List<ResultCardVisual>();

        foreach (var card in cards)
        {
            try
            {
                var vis = BuildResultCardVisual(card);
                _cardVisuals[card.ServiceInstanceId] = vis;
                ResultsPanel.Children.Add(vis.Root);

                // 结构重建时若从折叠→展开（首条 delta / finished），排队自动展开动画
                var wasCollapsed = prevCollapsed.GetValueOrDefault(card.ServiceInstanceId, true);
                if (wasCollapsed && !card.Collapsed)
                {
                    // 先以折叠态入树，下一帧再动画展开
                    vis.BodyHost.MaxHeight = 0;
                    vis.BodyHost.Opacity = 0;
                    vis.BodyHost.Clip = new RectangleGeometry { Rect = new Windows.Foundation.Rect(0, 0, 0, 0) };
                    if (vis.CollapseChevron.RenderTransform is RotateTransform rot)
                        rot.Angle = -90;
                    else
                    {
                        vis.CollapseChevron.RenderTransform = new RotateTransform { Angle = -90 };
                        vis.CollapseChevron.RenderTransformOrigin = new Windows.Foundation.Point(0.5, 0.5);
                    }

                    vis.Collapsed = true; // 动画完成后与 card 对齐
                    pendingExpand.Add(vis);
                }
            }
            catch (Exception ex)
            {
                CrashLog.Write($"RebuildResultCards.Build[{card.ServiceInstanceId}]", ex);
            }
        }

        if (pendingExpand.Count == 0)
            return;

        // 等入树 arrange 后再播展开动画（否则 Measure fullH 不准）
        _ = DispatcherQueue.TryEnqueue(() =>
        {
            foreach (var vis in pendingExpand)
            {
                if (!_bridge.State.Cards.TryGetValue(vis.Id, out var live) || live.Collapsed)
                    continue;
                AnimateCardBodyCollapse(vis.BodyHost, vis.Body, vis.CollapseChevron, collapsing: false);
                vis.Collapsed = false;
            }
        });
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

    private ResultCardVisual BuildResultCardVisual(CardState card)
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

        // 原型：box-shadow 常态/hover；另有 ::before reveal 高光（用 hover 抬升 + 描边近似）
        ApplyCardElevation(root, elevated: false);
        root.PointerEntered += (_, _) =>
        {
            root.BorderBrush = border2;
            ApplyCardElevation(root, elevated: true);
        };
        root.PointerExited += (_, _) =>
        {
            root.BorderBrush = borderBrush;
            ApplyCardElevation(root, elevated: false);
        };

        var stack = new StackPanel { Spacing = 0 };
        var serviceId = card.ServiceInstanceId;
        var displayName = string.IsNullOrWhiteSpace(card.ServiceName) ? card.ServiceInstanceId : card.ServiceName;

        // —— header: padding 6 12；icon 14 + name 11 Medium + status + collapse 20 ——
        var header = new Grid { Padding = new Thickness(12, 6, 12, 6) };
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        // 与设置页 ServiceIcon 同源：Assets/service-icons/{serviceType}.svg；原型 14×14 r=3
        var iconHost = new Border
        {
            Width = 14,
            Height = 14,
            CornerRadius = new CornerRadius(3),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 6, 0),
            Child = ServiceIcons.Create(card.ServiceType, displayName, 14),
        };
        Grid.SetColumn(iconHost, 0);
        header.Children.Add(iconHost);

        // 原型 .result-engine-name：0.6875rem=11px / font-weight 500
        var title = new TextBlock
        {
            Text = displayName,
            FontFamily = PopupFont(),
            FontWeight = Microsoft.UI.Text.FontWeights.Medium,
            FontSize = PopupFontSize("PopupFontSizeEngine", 11),
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
        // 展开态用大有限 MaxHeight（见 BodyExpandedMax 注释）
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

        var visual = new ResultCardVisual
        {
            Id = serviceId,
            Root = root,
            BodyHost = bodyHost,
            Body = body,
            CollapseChevron = collapseIcon,
            CollapseButton = collapseBtn,
            TextClip = textClip,
            ResultText = FindResultTextBlock(textClip),
            Collapsed = card.Collapsed,
            StructureKey = CardStructureKey(card),
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
            if (_cardVisuals.TryGetValue(serviceId, out var v))
                v.Collapsed = collapsing;
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
        return visual;
    }

    private static TextBlock? FindResultTextBlock(Border? textClip)
    {
        if (textClip is null)
            return null;
        if (textClip.Child is TextBlock direct)
            return direct;
        if (textClip.Child is Panel panel)
        {
            foreach (var child in panel.Children)
            {
                if (child is TextBlock tb)
                    return tb;
                if (child is Panel nested)
                {
                    foreach (var n in nested.Children)
                    {
                        if (n is TextBlock ntb)
                            return ntb;
                    }
                }
            }
        }

        return null;
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

    /// <summary>原文/译文共用：Segoe UI Variable + 13px + line-height 1.6（原型 .result-text）。</summary>
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
            // 原型 font-feature-settings: ss01 + tnum；WinUI 侧靠 Variable 字体默认 metric
            OpticalMarginAlignment = OpticalMarginAlignment.TrimSideBearings,
        };
    }

    private static FontFamily PopupFont() =>
        TryResourceFont("PopupFontFamily")
        ?? new FontFamily("Segoe UI Variable, Segoe UI, Microsoft YaHei UI, Microsoft YaHei");

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
    /// <param name="force">为 true 时忽略高度阈值（动画终帧必须上报宿主）。</param>
    private void ReportContentSize(bool notifyHost = true, bool force = false)
    {
        if (_sizeReportBusy || !IsPopupVisible)
            return;
        _sizeReportBusy = true;
        try
        {
            var h = MeasureDesiredWindowLogicalHeight();
            const double w = PopupLogicalWidth;

            // 跟窗中阈值更小，稳态略大以合并布局抖动
            var threshold = _sizeFollowActive ? 0.35 : 1.5;
            if (!force && Math.Abs(h - _lastReportedHeight) < threshold)
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

    /// <summary>
    /// 对齐 Vue <c>popupRef.offsetHeight</c>：用已排版的实际高度求和（内容驱动）。
    /// 中间行 Auto 后 ContentPanel.ActualHeight 随 MaxHeight 动画真实变化，勿每帧强行 Measure
    ///（会与 dependent animation 抢布局，产生终帧跳动）。
    /// </summary>
    private double MeasureDesiredWindowLogicalHeight()
    {
        var statusH = StatusBarBorder?.ActualHeight > 1
            ? StatusBarBorder.ActualHeight
            : 40;

        // 超高内容时让 ScrollViewer 在上限内滚动，而不是把窗撑破
        var maxBody = Math.Max(80, PopupMaxLogicalHeight - 44 - statusH - 2);
        if (BodyScrollViewer is not null
            && (double.IsNaN(BodyScrollViewer.MaxHeight) || Math.Abs(BodyScrollViewer.MaxHeight - maxBody) > 0.5))
        {
            BodyScrollViewer.MaxHeight = maxBody;
        }

        double contentH;
        if (ContentPanel.ActualHeight > 1)
        {
            // 动画跟窗：读真实排版高（≈ offsetHeight 路径）
            contentH = ContentPanel.ActualHeight;
        }
        else
        {
            // 冷启动尚未 arrange：才 Measure
            var contentWidth = ContentPanel.ActualWidth > 1
                ? ContentPanel.ActualWidth
                : PopupLogicalWidth - 28;
            ContentPanel.Measure(new Windows.Foundation.Size(contentWidth, double.PositiveInfinity));
            contentH = ContentPanel.DesiredSize.Height;
        }

        if (double.IsNaN(contentH) || double.IsInfinity(contentH) || contentH < 1)
            contentH = 120;

        // 标题 44 + 内容 + 状态栏 + ShellBorder 上下边 1px*2
        var h = 44 + contentH + statusH + 2;
        return Math.Clamp(h, PopupMinLogicalHeight, PopupMaxLogicalHeight);
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
        // 物理像素高度未变则跳过，避免 DPI 下无效 Resize 闪动
        if (h == _lastResizePhysicalH)
            return;

        _lastResizePhysicalH = h;
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
        // 跟窗期间由 CompositionTarget 采样，禁止 SizeChanged 二次改尺寸
        if (_sizeFollowActive)
            return;
        _sizeDebounce?.Stop();
        _sizeDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(80) };
        _sizeDebounce.Tick += (_, _) =>
        {
            _sizeDebounce?.Stop();
            if (_sizeFollowActive)
                return;
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
        _sourceCardFocused = focused;
        UpdateSourceCardChrome();
    }

    private void SourceCardBorder_PointerEntered(object sender, PointerRoutedEventArgs e)
    {
        _sourceCardHovered = true;
        UpdateSourceCardChrome();
    }

    private void SourceCardBorder_PointerExited(object sender, PointerRoutedEventArgs e)
    {
        _sourceCardHovered = false;
        UpdateSourceCardChrome();
    }

    /// <summary>
    /// 源文卡描边 + 阴影：对齐原型 focus-within accent ring + shadow-card / shadow-card-h。
    /// </summary>
    private void UpdateSourceCardChrome()
    {
        if (SourceCardBorder is null)
            return;

        if (_sourceCardFocused)
        {
            SourceCardBorder.BorderBrush = TryResourceBrush("PopupAccentBrush")
                ?? new SolidColorBrush(Color.FromArgb(255, 0xD5, 0x5A, 0x1F));
            SourceCardBorder.BorderThickness = new Thickness(1.5);
            ApplyCardElevation(SourceCardBorder, elevated: true);
        }
        else
        {
            SourceCardBorder.BorderBrush = _sourceCardHovered
                ? (TryResourceBrush("PopupBorder2Brush")
                   ?? new SolidColorBrush(Color.FromArgb(0x1F, 0, 0, 0)))
                : (TryResourceBrush("PopupBorderBrush")
                   ?? new SolidColorBrush(Color.FromArgb(0x0F, 0, 0, 0)));
            SourceCardBorder.BorderThickness = new Thickness(1);
            ApplyCardElevation(SourceCardBorder, elevated: _sourceCardHovered);
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
                // 与 BuildResultCard 一致：大有限值，禁止 ClearValue/Infinity 污染 Measure
                bodyHost.MaxHeight = BodyExpandedMax;
                bodyHost.Opacity = 1;
                bodyHost.Clip = null;
            }

            if (--_pendingBodySizeAnims <= 0)
            {
                _pendingBodySizeAnims = 0;
                FinishContentSizeFollow(notifyHost: true);
            }
        };
        // 先计数再 Begin，避免 Completed 同步触发时计数错乱
        _pendingBodySizeAnims++;
        sb.Begin();

        // 对齐 Vue：CSS 变高 → ResizeObserver 读 offsetHeight → setSize
        StartContentSizeFollow();
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
            FinishContentSizeFollow(notifyHost: true);
        };
        sb.Begin();
        StartContentSizeFollow();
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
    /// 卡片 MaxHeight 动画期间：每帧量测壳层期望高并本地 Resize（对齐 Vue ResizeObserver + rAF）。
    /// 不并行猜 Δ，避免与真实布局脱节后在终帧猛拉。
    /// 多卡同时动画时只挂一次 Rendering，由 _pendingBodySizeAnims 决定何时收尾。
    /// </summary>
    private void StartContentSizeFollow()
    {
        _sizeDebounce?.Stop();
        if (_sizeFollowActive)
        {
            ReportContentSize(notifyHost: false);
            return;
        }

        _sizeFollowActive = true;
        _sizeFollowRenderHandler ??= OnContentSizeFollowRender;
        CompositionTarget.Rendering += _sizeFollowRenderHandler;
        // 首帧立刻跟一次，避免等下一 Rendering
        ReportContentSize(notifyHost: false);
    }

    private void OnContentSizeFollowRender(object? sender, object e)
    {
        if (!_sizeFollowActive)
            return;
        ReportContentSize(notifyHost: false);
    }

    /// <summary>Storyboard 结束后停跟窗，再统一 report 宿主一次。</summary>
    private void FinishContentSizeFollow(bool notifyHost)
    {
        if (_sizeFollowRenderHandler is not null)
            CompositionTarget.Rendering -= _sizeFollowRenderHandler;

        _sizeDebounce?.Stop();
        _sizeFollowActive = false;
        _pendingBodySizeAnims = 0;

        try
        {
            ShellBorder.UpdateLayout();
            ContentPanel.UpdateLayout();
        }
        catch
        {
            // ignore
        }

        // 与动画期同一测高路径；force 保证宿主拿到终态
        ReportContentSize(notifyHost: notifyHost, force: true);
    }

    private void ScheduleReportContentSize()
    {
        if (_sizeFollowActive)
            return;

        _sizeDebounce?.Stop();
        _sizeDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(40) };
        _sizeDebounce.Tick += (_, _) =>
        {
            _sizeDebounce?.Stop();
            if (_sizeFollowActive)
                return;
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
