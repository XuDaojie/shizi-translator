using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Shizi.Popup.Data;
using Shizi.Popup.Services;
using Shizi.Popup.State;
using Windows.ApplicationModel.DataTransfer;
using Windows.Graphics;
using Windows.System;
using WinRT.Interop;

namespace Shizi.Popup;

public sealed partial class MainWindow : Window
{
    private readonly AppWindow _appWindow;
    private readonly BridgeService _bridge = BridgeService.Instance;
    private readonly SpeechService _speech = new();
    private bool _alwaysOnTop;
    private bool _suppressLangEvents;
    private bool _suppressSourceTextEvents;
    private bool _startupStarted;
    private bool _sizeReportBusy;
    private double _lastReportedHeight;
    private DispatcherTimer? _tipTimer;
    private DispatcherTimer? _sizeDebounce;

    public MainWindow()
    {
        InitializeComponent();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        var hwnd = WindowNative.GetWindowHandle(this);
        var windowId = Win32Interop.GetWindowIdFromWindow(hwnd);
        _appWindow = AppWindow.GetFromWindowId(windowId);

        _appWindow.IsShownInSwitchers = false;

        if (_appWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
            presenter.SetBorderAndTitleBar(true, false);
        }

        TryResizeLogical(420, 280);

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

        _appWindow.Show();
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
        // 不要在 IPC 同步 Show 路径内立刻 report_content_size：
        // 否则 request 可能先于 show 的 result 到达 Rust，嵌套 set_size 与
        // 外层 show 的 OP_GATE 形成死锁/超时，导致 Rust 回退 webview。
        _ = DispatcherQueue.TryEnqueue(() => ReportContentSize());
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
        _suppressLangEvents = true;
        try
        {
            SourceLangCombo.ItemsSource = TranslationLanguages.All.ToList();
            SourceLangCombo.DisplayMemberPath = nameof(TranslationLanguage.NativeName);
            TargetLangCombo.ItemsSource = TranslationLanguages.Targets.ToList();
            TargetLangCombo.DisplayMemberPath = nameof(TranslationLanguage.NativeName);
            SelectLangCombo(SourceLangCombo, _bridge.SessionSourceLang);
            SelectLangCombo(TargetLangCombo, _bridge.SessionTargetLang);
        }
        finally
        {
            _suppressLangEvents = false;
        }
    }

    private void ApplyLocalizedChrome()
    {
        Title = Localization.T("window.popupTitle");
        SourceTextBox.PlaceholderText = Localization.T("popup.source.placeholder");
        TranslateButton.Content = Localization.T("popup.action.translate");
        ToolTipService.SetToolTip(PinButton, Localization.T(_alwaysOnTop ? "popup.tooltip.unpin" : "popup.tooltip.pin"));
        ToolTipService.SetToolTip(OcrButton, Localization.T("popup.tooltip.ocr"));
        ToolTipService.SetToolTip(SettingsButton, Localization.T("popup.tooltip.settings"));
        ToolTipService.SetToolTip(BookmarkButton, Localization.T("popup.tooltip.bookmark"));
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

        // 语言
        _suppressLangEvents = true;
        try
        {
            SelectLangCombo(SourceLangCombo, _bridge.SessionSourceLang);
            SelectLangCombo(TargetLangCombo, _bridge.SessionTargetLang);
        }
        finally
        {
            _suppressLangEvents = false;
        }

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
        StatusRing.IsActive = _bridge.StatusLoading;
        StatusRing.Visibility = _bridge.StatusLoading ? Visibility.Visible : Visibility.Collapsed;

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

        TranslateButton.IsEnabled = !_bridge.IsTranslating;
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
        var root = new Border
        {
            Background = TryThemeBrush("CardBackgroundFillColorDefaultBrush"),
            BorderBrush = TryThemeBrush("CardStrokeColorDefaultBrush"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(10, 8, 10, 8),
        };

        var stack = new StackPanel { Spacing = 6 };

        // 标题行
        var header = new Grid();
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var title = new TextBlock
        {
            Text = string.IsNullOrWhiteSpace(card.ServiceName) ? card.ServiceInstanceId : card.ServiceName,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
            FontSize = 13,
            VerticalAlignment = VerticalAlignment.Center,
            TextWrapping = TextWrapping.NoWrap,
        };
        if (!string.IsNullOrWhiteSpace(card.ModelName) && card.Protocol != "microsoft_edge")
        {
            title.Text = $"{title.Text} · {card.ModelName}";
        }

        var headerButtons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 0 };

        var collapseBtn = new Button
        {
            Width = 28,
            Height = 28,
            Padding = new Thickness(0),
            Content = new FontIcon
            {
                Glyph = card.Collapsed ? "\uE70D" : "\uE70E",
                FontSize = 12,
            },
        };
        collapseBtn.Background = new SolidColorBrush(Colors.Transparent);
        collapseBtn.BorderThickness = new Thickness(0);
        ToolTipService.SetToolTip(collapseBtn, Localization.T(card.Collapsed ? "popup.tooltip.expand" : "popup.tooltip.collapse"));
        var serviceId = card.ServiceInstanceId;
        collapseBtn.Click += (_, _) => _bridge.ToggleCardCollapsed(serviceId);
        headerButtons.Children.Add(collapseBtn);

        if (card.Status == CardStatus.Finished || !string.IsNullOrEmpty(card.Text))
        {
            var copyBtn = MakeIconButton("\uE8C8", Localization.T("popup.tooltip.copy"));
            var textSnapshot = card.Text;
            copyBtn.Click += (_, _) => CopyText(textSnapshot);
            headerButtons.Children.Add(copyBtn);

            var speakBtn = MakeIconButton("\uE767", Localization.T("popup.tooltip.speak"));
            speakBtn.Click += async (_, _) =>
            {
                try { await _speech.SpeakAsync(textSnapshot); }
                catch { /* ignore */ }
            };
            headerButtons.Children.Add(speakBtn);
        }

        if (card.Status == CardStatus.Failed)
        {
            var retryBtn = MakeIconButton("\uE72C", Localization.T("popup.tooltip.retry"));
            retryBtn.Click += (_, _) => _bridge.RetryTranslation();
            headerButtons.Children.Add(retryBtn);
        }

        Grid.SetColumn(headerButtons, 1);
        header.Children.Add(title);
        header.Children.Add(headerButtons);
        stack.Children.Add(header);

        // 正文 / 错误
        if (!card.Collapsed)
        {
            if (card.Status == CardStatus.Failed)
            {
                var errTitle = Localization.T(card.ErrorTitleKey ?? "popup.error.translationFailed");
                stack.Children.Add(new TextBlock
                {
                    Text = errTitle,
                    Foreground = TryThemeBrush("SystemFillColorCriticalBrush")
                        ?? new SolidColorBrush(Colors.IndianRed),
                    FontSize = 12,
                    FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                });
                if (!string.IsNullOrEmpty(card.ErrorMessage))
                {
                    stack.Children.Add(new TextBlock
                    {
                        Text = card.ErrorMessage,
                        FontSize = 12,
                        Opacity = 0.85,
                        TextWrapping = TextWrapping.WrapWholeWords,
                    });
                }
            }
            else if (card.Status == CardStatus.Cancelled)
            {
                stack.Children.Add(new TextBlock
                {
                    Text = Localization.T(card.ErrorTitleKey ?? "popup.status.cancelled"),
                    FontSize = 12,
                    Opacity = 0.7,
                });
                if (!string.IsNullOrEmpty(card.Text))
                {
                    stack.Children.Add(new TextBlock
                    {
                        Text = card.Text,
                        FontSize = 14,
                        TextWrapping = TextWrapping.WrapWholeWords,
                    });
                }
            }
            else if (card.Status == CardStatus.Translating && string.IsNullOrEmpty(card.Text))
            {
                var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                row.Children.Add(new ProgressRing { Width = 16, Height = 16, IsActive = true });
                row.Children.Add(new TextBlock
                {
                    Text = Localization.T("popup.status.translating"),
                    FontSize = 12,
                    Opacity = 0.7,
                    VerticalAlignment = VerticalAlignment.Center,
                });
                stack.Children.Add(row);
            }
            else if (card.Status == CardStatus.Pending)
            {
                stack.Children.Add(new TextBlock
                {
                    Text = "…",
                    FontSize = 12,
                    Opacity = 0.45,
                });
            }
            else
            {
                stack.Children.Add(new TextBlock
                {
                    Text = card.Text,
                    FontSize = 14,
                    TextWrapping = TextWrapping.WrapWholeWords,
                    IsTextSelectionEnabled = true,
                });
            }

            if (card.Usage is not null && card.ShowActions)
            {
                stack.Children.Add(new TextBlock
                {
                    Text = $"Tokens {card.Usage.InputTokens} → {card.Usage.OutputTokens}",
                    FontSize = 11,
                    Opacity = 0.5,
                });
            }
        }

        root.Child = stack;
        return root;
    }

    private static Button MakeIconButton(string glyph, string tip)
    {
        var btn = new Button
        {
            Width = 28,
            Height = 28,
            Padding = new Thickness(0),
            Content = new FontIcon { Glyph = glyph, FontSize = 12 },
        };
        btn.Background = new SolidColorBrush(Colors.Transparent);
        btn.BorderThickness = new Thickness(0);
        ToolTipService.SetToolTip(btn, tip);
        return btn;
    }

    private static void SelectLangCombo(ComboBox combo, string code)
    {
        if (combo.ItemsSource is not System.Collections.IEnumerable items)
            return;

        TranslationLanguage? match = null;
        foreach (var item in items)
        {
            if (item is TranslationLanguage lang &&
                string.Equals(lang.Code, code, StringComparison.OrdinalIgnoreCase))
            {
                match = lang;
                break;
            }
        }

        if (match is not null)
            combo.SelectedItem = match;
        else if (combo.Items.Count > 0)
            combo.SelectedIndex = 0;
    }

    private void UpdatePinButtonVisual()
    {
        ToolTipService.SetToolTip(PinButton, Localization.T(_alwaysOnTop ? "popup.tooltip.unpin" : "popup.tooltip.pin"));
        if (PinButton.Content is FontIcon icon)
        {
            // 置顶高亮：换用 filled 风格近似
            icon.Glyph = _alwaysOnTop ? "\uE841" : "\uE840";
            icon.Foreground = _alwaysOnTop
                ? TryThemeBrush("AccentTextFillColorPrimaryBrush")
                : TryThemeBrush("TextFillColorPrimaryBrush");
        }
    }

    private static Brush? TryThemeBrush(string key)
    {
        try
        {
            if (Application.Current.Resources.TryGetValue(key, out var v) && v is Brush b)
                return b;
        }
        catch
        {
            // ignore
        }

        return null;
    }

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
            // 内容期望高度：量测 + 顶栏 + 状态栏边距
            ContentPanel.Measure(new Windows.Foundation.Size(396, double.PositiveInfinity));
            var contentH = ContentPanel.DesiredSize.Height;
            var h = Math.Clamp(contentH + 40 + 44 + 24, 160, 720);
            const double w = 420;

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

    private void BookmarkButton_Click(object sender, RoutedEventArgs e) =>
        ShowTipBar(Localization.T("popup.toast.featureWip"));

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

    private void TranslateButton_Click(object sender, RoutedEventArgs e) =>
        _bridge.StartTranslation(SourceTextBox.Text);

    private void SourceLangCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressLangEvents)
            return;
        if (SourceLangCombo.SelectedItem is TranslationLanguage lang)
            _bridge.SetSessionLanguages(lang.Code, _bridge.SessionTargetLang);
    }

    private void TargetLangCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressLangEvents)
            return;
        if (TargetLangCombo.SelectedItem is TranslationLanguage lang)
            _bridge.SetSessionLanguages(_bridge.SessionSourceLang, lang.Code);
    }

    private void SwapLang_Click(object sender, RoutedEventArgs e)
    {
        if (!_bridge.TrySwapLanguages())
        {
            // tip 已由 Bridge 设置
            if (!string.IsNullOrEmpty(_bridge.TipMessage))
            {
                ShowTipBar(_bridge.TipMessage);
                _bridge.ClearTip();
            }
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
