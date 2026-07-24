namespace Shizi.Popup.Services;

/// <summary>
/// 弹窗文案（popup.* 语义）。当前内置中文；interface_language_changed 可切换键表。
/// </summary>
public static class Localization
{
    private static string _locale = "zh-CN";

    private static readonly Dictionary<string, string> Zh = new(StringComparer.Ordinal)
    {
        ["popup.status.ready"] = "就绪",
        ["popup.status.translating"] = "翻译中…",
        ["popup.status.completed"] = "翻译完成",
        ["popup.status.failed"] = "翻译失败",
        ["popup.status.partial"] = "部分完成",
        ["popup.status.cancelled"] = "已取消",
        ["popup.status.detecting"] = "检测中…",
        ["popup.error.translationFailed"] = "翻译失败",
        ["popup.error.swapAuto"] = "自动检测不支持交换",
        ["popup.error.emptySource"] = "请输入要翻译的文本",
        ["popup.error.openSettings"] = "打开设置失败",
        ["popup.error.cancelFailed"] = "取消失败",
        ["popup.error.retryFailed"] = "重试失败",
        ["popup.error.languageSaveFailed"] = "语言保存失败",
        ["popup.error.pinFailed"] = "置顶失败",
        ["popup.error.ocrFailed"] = "截图译失败",
        ["popup.error.pendingSourceFailed"] = "获取待译原文失败",
        ["popup.toast.pinned"] = "已置顶",
        ["popup.toast.unpinned"] = "已取消置顶",
        ["popup.toast.copied"] = "已复制",
        ["popup.toast.featureWip"] = "功能开发中",
        ["popup.tooltip.pin"] = "置顶",
        ["popup.tooltip.unpin"] = "取消置顶",
        ["popup.tooltip.ocr"] = "截图翻译",
        ["popup.tooltip.settings"] = "设置",
        ["popup.tooltip.bookmark"] = "收藏",
        ["popup.tooltip.copy"] = "复制",
        ["popup.tooltip.speak"] = "朗读",
        ["popup.tooltip.collapse"] = "折叠",
        ["popup.tooltip.expand"] = "展开",
        ["popup.tooltip.retry"] = "重试",
        ["popup.tooltip.swap"] = "交换语言",
        ["popup.action.cancel"] = "取消",
        ["popup.action.retry"] = "重试",
        ["popup.action.translate"] = "翻译",
        ["popup.charCount"] = "{count} 字",
        ["popup.badge.selectedText"] = "划词",
        ["popup.badge.ocrText"] = "OCR",
        ["popup.badge.manualText"] = "手动",
        ["popup.source.placeholder"] = "输入或粘贴要翻译的文本…",
        ["window.popupTitle"] = "Shizi 翻译",
    };

    private static readonly Dictionary<string, string> En = new(StringComparer.Ordinal)
    {
        ["popup.status.ready"] = "Ready",
        ["popup.status.translating"] = "Translating…",
        ["popup.status.completed"] = "Done",
        ["popup.status.failed"] = "Failed",
        ["popup.status.partial"] = "Partial",
        ["popup.status.cancelled"] = "Cancelled",
        ["popup.status.detecting"] = "Detecting…",
        ["popup.error.translationFailed"] = "Translation failed",
        ["popup.error.swapAuto"] = "Cannot swap when source is Auto",
        ["popup.error.emptySource"] = "Enter text to translate",
        ["popup.error.openSettings"] = "Failed to open settings",
        ["popup.error.cancelFailed"] = "Cancel failed",
        ["popup.error.retryFailed"] = "Retry failed",
        ["popup.error.languageSaveFailed"] = "Failed to save languages",
        ["popup.error.pinFailed"] = "Pin failed",
        ["popup.error.ocrFailed"] = "OCR translate failed",
        ["popup.error.pendingSourceFailed"] = "Failed to take pending source",
        ["popup.toast.pinned"] = "Pinned",
        ["popup.toast.unpinned"] = "Unpinned",
        ["popup.toast.copied"] = "Copied",
        ["popup.toast.featureWip"] = "Coming soon",
        ["popup.tooltip.pin"] = "Pin",
        ["popup.tooltip.unpin"] = "Unpin",
        ["popup.tooltip.ocr"] = "Screenshot translate",
        ["popup.tooltip.settings"] = "Settings",
        ["popup.tooltip.bookmark"] = "Bookmark",
        ["popup.tooltip.copy"] = "Copy",
        ["popup.tooltip.speak"] = "Speak",
        ["popup.tooltip.collapse"] = "Collapse",
        ["popup.tooltip.expand"] = "Expand",
        ["popup.tooltip.retry"] = "Retry",
        ["popup.tooltip.swap"] = "Swap languages",
        ["popup.action.cancel"] = "Cancel",
        ["popup.action.retry"] = "Retry",
        ["popup.action.translate"] = "Translate",
        ["popup.charCount"] = "{count} chars",
        ["popup.badge.selectedText"] = "Selection",
        ["popup.badge.ocrText"] = "OCR",
        ["popup.badge.manualText"] = "Manual",
        ["popup.source.placeholder"] = "Type or paste text to translate…",
        ["window.popupTitle"] = "Shizi Translate",
    };

    public static string Locale => _locale;

    public static void SetLocale(string? locale)
    {
        if (string.IsNullOrWhiteSpace(locale) || locale == "auto")
        {
            _locale = "zh-CN";
            return;
        }

        _locale = locale.StartsWith("en", StringComparison.OrdinalIgnoreCase) ? "en" : "zh-CN";
    }

    public static string T(string key, IReadOnlyDictionary<string, string>? args = null)
    {
        var table = _locale.StartsWith("en", StringComparison.OrdinalIgnoreCase) ? En : Zh;
        if (!table.TryGetValue(key, out var s) && !Zh.TryGetValue(key, out s))
            s = key;

        if (args is not null)
        {
            foreach (var (k, v) in args)
                s = s.Replace($"{{{k}}}", v, StringComparison.Ordinal);
        }

        return s;
    }

    public static string T(string key, string argName, string argValue) =>
        T(key, new Dictionary<string, string> { [argName] = argValue });
}
