using System.Runtime.InteropServices;
using Microsoft.UI;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Shizi.Popup.Data;
using Shizi.Popup.Services;
using Shizi.Popup.State;
using Windows.ApplicationModel.DataTransfer;
using Windows.Graphics;
using Windows.System;
using Windows.UI;
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

    public MainWindow()
    {
        InitializeComponent();

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

    /// <summary>Mica 优先（shell 透明让材质透出）；不可用时 solid 近似。</summary>
    private void TryApplyMicaBackdrop()
    {
        try
        {
            if (MicaController.IsSupported())
            {
                SystemBackdrop = new MicaBackdrop
                {
                    Kind = MicaKind.Base,
                };
                RootGrid.Background = new SolidColorBrush(Colors.Transparent);
                // 材质由 SystemBackdrop 提供；shell 仅保留细边与圆角裁剪
                ShellBorder.Background = new SolidColorBrush(Colors.Transparent);
                return;
            }
        }
        catch
        {
            // fall through
        }

        RootGrid.Background = new SolidColorBrush(Colors.Transparent);
        ShellBorder.Background = TryResourceBrush("PopupMicaFallbackBrush")
            ?? new SolidColorBrush(Color.FromArgb(0xEB, 0xF4, 0xF4, 0xF4));
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
            DispatcherQueue.TryEnqueue(() =>
            {
                RefreshFromBridge();
                ReportContentSize();
            });
        }
        catch
        {
            // ignore
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

        // 状态点：翻译中 accent；完成 success；失败 destructive；其它 accent 静态
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
        ResultsPanel.Children.Clear();
        foreach (var id in _bridge.State.CardOrder)
        {
            if (!_bridge.State.Cards.TryGetValue(id, out var card))
                continue;
            ResultsPanel.Children.Add(BuildResultCard(card));
        }
    }

    private UIElement BuildResultCard(CardState card)
    {
        var cardBg = TryResourceBrush("PopupCardBgBrush")
            ?? new SolidColorBrush(Color.FromArgb(0xB8, 255, 255, 255));
        var borderBrush = TryResourceBrush("PopupBorderBrush")
            ?? new SolidColorBrush(Color.FromArgb(0x0F, 0, 0, 0));
        var fg = TryResourceBrush("PopupFgBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0x1A, 0x1A, 0x1A));
        var fg2 = TryResourceBrush("PopupFg2Brush") ?? new SolidColorBrush(Color.FromArgb(255, 0x5D, 0x5D, 0x5D));
        var fg3 = TryResourceBrush("PopupFg3Brush") ?? new SolidColorBrush(Color.FromArgb(255, 0x8A, 0x8A, 0x8A));
        var accent = TryResourceBrush("PopupAccentBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0xD5, 0x5A, 0x1F));
        var danger = TryResourceBrush("PopupDestructiveBrush") ?? new SolidColorBrush(Color.FromArgb(255, 0xC4, 0x2B, 0x1C));
        var onAccent = TryResourceBrush("PopupOnAccentBrush") ?? new SolidColorBrush(Colors.White);

        var root = new Border
        {
            Background = cardBg,
            BorderBrush = borderBrush,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(0, 0, 0, 0),
        };

        var stack = new StackPanel { Spacing = 0 };

        // —— 头：图标 + 名 + 状态点 + 折叠 ——
        var header = new Grid
        {
            Padding = new Thickness(12, 6, 8, 6),
        };
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var displayName = string.IsNullOrWhiteSpace(card.ServiceName) ? card.ServiceInstanceId : card.ServiceName;
        var initial = string.IsNullOrEmpty(displayName)
            ? "?"
            : displayName.Trim()[0].ToString().ToUpperInvariant();

        var icon = new Border
        {
            Width = 14,
            Height = 14,
            CornerRadius = new CornerRadius(3),
            Background = accent,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 6, 0),
            Child = new TextBlock
            {
                Text = initial,
                FontSize = 9,
                FontWeight = Microsoft.UI.Text.FontWeights.Bold,
                Foreground = onAccent,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };
        Grid.SetColumn(icon, 0);
        header.Children.Add(icon);

        var title = new TextBlock
        {
            Text = displayName,
            FontWeight = Microsoft.UI.Text.FontWeights.Medium,
            FontSize = 11,
            Foreground = fg2,
            VerticalAlignment = VerticalAlignment.Center,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        Grid.SetColumn(title, 1);
        header.Children.Add(title);

        // 状态点：翻译中 / 失败 / pending
        if (card.Status is CardStatus.Translating or CardStatus.Pending or CardStatus.Failed)
        {
            var dot = new Ellipse
            {
                Width = 6,
                Height = 6,
                Fill = card.Status == CardStatus.Failed ? danger : accent,
                Margin = new Thickness(4, 0, 4, 0),
                VerticalAlignment = VerticalAlignment.Center,
            };
            Grid.SetColumn(dot, 2);
            header.Children.Add(dot);
        }

        var collapseBtn = MakeIconButton(
            card.Collapsed ? "\uE70D" : "\uE70E",
            Localization.T(card.Collapsed ? "popup.tooltip.expand" : "popup.tooltip.collapse"),
            20,
            11);
        var serviceId = card.ServiceInstanceId;
        collapseBtn.Click += (_, _) => _bridge.ToggleCardCollapsed(serviceId);
        Grid.SetColumn(collapseBtn, 3);
        header.Children.Add(collapseBtn);

        // 点击头折叠（除按钮外）
        header.PointerPressed += (_, e) =>
        {
            if (e.OriginalSource is DependencyObject src && IsDescendantOf(src, collapseBtn))
                return;
            _bridge.ToggleCardCollapsed(serviceId);
        };

        stack.Children.Add(header);

        // —— 身 + 底栏 ——
        if (!card.Collapsed)
        {
            var body = new StackPanel
            {
                Spacing = 6,
                Padding = new Thickness(12, 0, 12, 9),
            };

            if (card.Status == CardStatus.Failed)
            {
                var errTitle = Localization.T(card.ErrorTitleKey ?? "popup.error.translationFailed");
                body.Children.Add(new TextBlock
                {
                    Text = errTitle,
                    Foreground = danger,
                    FontSize = 12,
                    FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                });
                if (!string.IsNullOrEmpty(card.ErrorMessage))
                {
                    body.Children.Add(new TextBlock
                    {
                        Text = card.ErrorMessage,
                        FontSize = 12,
                        Foreground = fg2,
                        TextWrapping = TextWrapping.WrapWholeWords,
                    });
                }
            }
            else if (card.Status == CardStatus.Cancelled)
            {
                body.Children.Add(new TextBlock
                {
                    Text = Localization.T(card.ErrorTitleKey ?? "popup.status.cancelled"),
                    FontSize = 12,
                    Foreground = fg3,
                });
                if (!string.IsNullOrEmpty(card.Text))
                {
                    body.Children.Add(new TextBlock
                    {
                        Text = card.Text,
                        FontSize = 14,
                        Foreground = fg,
                        TextWrapping = TextWrapping.WrapWholeWords,
                        IsTextSelectionEnabled = true,
                    });
                }
            }
            else if (card.Status == CardStatus.Translating && string.IsNullOrEmpty(card.Text))
            {
                body.Children.Add(new TextBlock
                {
                    Text = "…",
                    FontSize = 14,
                    Foreground = fg3,
                });
            }
            else if (card.Status == CardStatus.Pending)
            {
                body.Children.Add(new TextBlock
                {
                    Text = "…",
                    FontSize = 12,
                    Foreground = fg3,
                    Opacity = 0.7,
                });
            }
            else
            {
                body.Children.Add(new TextBlock
                {
                    Text = card.Text,
                    // 原型 components.css .result-text: 0.8125rem ≈ 13
                    FontSize = 13,
                    Foreground = fg,
                    TextWrapping = TextWrapping.WrapWholeWords,
                    IsTextSelectionEnabled = true,
                    LineHeight = 21,
                });
            }

            // 底栏对齐 ResultCardView：左 speak/copy(/retry)，右 model + ↑in | ↓out
            // 头栏只有 引擎图标+名+状态点+折叠，绝不放复制/朗读/model
            var showMeta = card.Protocol != "microsoft_edge"
                && (!string.IsNullOrWhiteSpace(card.ModelName) || card.Usage is not null);
            var textSnapshot = card.Text ?? "";
            var canActOnText = !string.IsNullOrEmpty(textSnapshot)
                || card.Status is CardStatus.Finished;
            var showRetry = card.Status is CardStatus.Failed or CardStatus.Cancelled;
            var showLeftActions = canActOnText || showRetry;

            if (showLeftActions || showMeta)
            {
                var actions = new Grid { Margin = new Thickness(0, 6, 0, 0) };
                actions.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                actions.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

                var left = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 3,
                    VerticalAlignment = VerticalAlignment.Center,
                    HorizontalAlignment = HorizontalAlignment.Left,
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
                    // result-model-group：margin-left:auto；model-tag + tokens
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
                            FontSize = 10,
                            FontFamily = new FontFamily("Cascadia Mono, Consolas, Courier New"),
                            Foreground = fg3,
                            VerticalAlignment = VerticalAlignment.Center,
                            MaxWidth = 160,
                            TextTrimming = TextTrimming.CharacterEllipsis,
                        });
                    }

                    if (card.Usage is not null)
                    {
                        // ↑in | ↓out（非 "Tokens x → y"）
                        var tokens = new StackPanel
                        {
                            Orientation = Orientation.Horizontal,
                            Spacing = 6,
                            VerticalAlignment = VerticalAlignment.Center,
                        };
                        tokens.Children.Add(MakeTokenChip("\uE74A", card.Usage.InputTokens, fg3));
                        tokens.Children.Add(new Border
                        {
                            Width = 1,
                            Height = 9,
                            Background = borderBrush,
                            VerticalAlignment = VerticalAlignment.Center,
                        });
                        tokens.Children.Add(MakeTokenChip("\uE74B", card.Usage.OutputTokens, fg3));
                        right.Children.Add(tokens);
                    }

                    Grid.SetColumn(right, 1);
                    actions.Children.Add(right);
                }

                body.Children.Add(actions);
            }

            stack.Children.Add(body);
        }

        root.Child = stack;
        return root;
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

    /// <summary>原型 .result-tokens .tok：小箭头 + 数字。</summary>
    private static UIElement MakeTokenChip(string arrowGlyph, int value, Brush fg3)
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            VerticalAlignment = VerticalAlignment.Center,
        };
        row.Children.Add(new FontIcon
        {
            Glyph = arrowGlyph,
            FontSize = 9,
            Foreground = fg3,
            Opacity = 0.55,
            VerticalAlignment = VerticalAlignment.Center,
        });
        row.Children.Add(new TextBlock
        {
            Text = value.ToString(),
            FontSize = 10,
            FontFamily = new FontFamily("Cascadia Mono, Consolas, Courier New"),
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
        ToolTipService.SetToolTip(PinButton, Localization.T(_alwaysOnTop ? "popup.tooltip.unpin" : "popup.tooltip.pin"));
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

    private void ReportContentSize()
    {
        if (_sizeReportBusy)
            return;
        _sizeReportBusy = true;
        try
        {
            // 内容期望高度：量测 + 顶栏 + 状态栏边距（body 水平 pad 14*2）
            var contentWidth = PopupLogicalWidth - 28;
            ContentPanel.Measure(new Windows.Foundation.Size(contentWidth, double.PositiveInfinity));
            var contentH = ContentPanel.DesiredSize.Height;
            var h = Math.Clamp(contentH + 44 + 40 + 16, 160, 720);
            const double w = PopupLogicalWidth;

            if (Math.Abs(h - _lastReportedHeight) < 2)
                return;

            _lastReportedHeight = h;
            TryResizeLogical(w, h);
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
        var scale = Content?.XamlRoot?.RasterizationScale ?? GetDpiScaleFallback();
        var w = Math.Max(1, (int)Math.Round(width * scale));
        var h = Math.Max(1, (int)Math.Round(height * scale));
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

    private void SourceTextBox_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter)
        {
            var ctrl = Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Control);
            if (ctrl.HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down))
            {
                e.Handled = true;
                _bridge.StartTranslation(SourceTextBox.Text);
            }
        }
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
