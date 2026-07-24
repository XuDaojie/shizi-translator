namespace Shizi.Popup.Data;

/// <summary>翻译语言项，对齐 frontend/src/shared/translation-languages.ts。</summary>
public sealed class TranslationLanguage
{
    public TranslationLanguage(string code, string nativeName, string promptName)
    {
        Code = code;
        NativeName = nativeName;
        PromptName = promptName;
        NameKey = $"language.{code}";
    }

    public string Code { get; }
    public string NativeName { get; }
    public string PromptName { get; }
    public string NameKey { get; }

    public override string ToString() => NativeName;
}

/// <summary>
/// 目标语 19 种 + auto；源语言 = auto + Targets。
/// </summary>
public static class TranslationLanguages
{
    public static TranslationLanguage Auto { get; } =
        new("auto", "自动检测", "Auto Detect");

    public static IReadOnlyList<TranslationLanguage> Targets { get; } = new[]
    {
        new TranslationLanguage("zh-CN", "简体中文", "Chinese (Simplified)"),
        new TranslationLanguage("zh-TW", "繁體中文", "Chinese (Traditional)"),
        new TranslationLanguage("en", "English", "English"),
        new TranslationLanguage("ja", "日本語", "Japanese"),
        new TranslationLanguage("ko", "한국어", "Korean"),
        new TranslationLanguage("fr", "Français", "French"),
        new TranslationLanguage("de", "Deutsch", "German"),
        new TranslationLanguage("es", "Español", "Spanish"),
        new TranslationLanguage("pt", "Português", "Portuguese"),
        new TranslationLanguage("ru", "Русский", "Russian"),
        new TranslationLanguage("it", "Italiano", "Italian"),
        new TranslationLanguage("nl", "Nederlands", "Dutch"),
        new TranslationLanguage("pl", "Polski", "Polish"),
        new TranslationLanguage("tr", "Türkçe", "Turkish"),
        new TranslationLanguage("ar", "العربية", "Arabic"),
        new TranslationLanguage("th", "ภาษาไทย", "Thai"),
        new TranslationLanguage("vi", "Tiếng Việt", "Vietnamese"),
        new TranslationLanguage("id", "Bahasa Indonesia", "Indonesian"),
        new TranslationLanguage("hi", "हिन्दी", "Hindi"),
    };

    /// <summary>源语言列表：auto + 全部目标语。</summary>
    public static IReadOnlyList<TranslationLanguage> All { get; } =
        new[] { Auto }.Concat(Targets).ToList();

    public static TranslationLanguage? Find(string? code)
    {
        if (string.IsNullOrEmpty(code))
            return null;
        return All.FirstOrDefault(l => string.Equals(l.Code, code, StringComparison.OrdinalIgnoreCase));
    }

    public static string DisplayName(string? code) =>
        Find(code)?.NativeName ?? code ?? "";
}
