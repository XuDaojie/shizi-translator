namespace Shizi.Popup.Data;

/// <summary>
/// 服务渠道 type → 品牌 SVG 文件名，与设置页 <c>ServiceIcon</c> /
/// <c>getServiceIconifyId</c> + <c>getServiceLogoSrc</c> 对齐。
/// 纯数据，无 WinUI 依赖（可单测）。
/// </summary>
public static class ServiceIconMap
{
    private static readonly Dictionary<string, string> FileByServiceType =
        new(StringComparer.OrdinalIgnoreCase)
        {
            ["openai"] = "openai.svg",
            ["deepseek"] = "deepseek.svg",
            ["claude"] = "claude.svg",
            ["anthropic"] = "anthropic.svg",
            ["microsoft"] = "microsoft.svg",
            ["gemini"] = "google.svg",
            ["deepl"] = "deepl.svg",
            ["google"] = "googletranslate.svg",
            ["baidu"] = "baidu.svg",
            ["tencent"] = "tencent.svg",
            ["volcengine"] = "volcengine.svg",
            ["zhipu"] = "zhipu.svg",
            ["moonshot"] = "moonshotai.svg",
            ["siliconflow"] = "siliconflow.svg",
            ["siliconcloud"] = "siliconflow.svg",
            ["edge"] = "microsoft.svg",
        };

    /// <summary>解析服务 type 对应的 SVG 文件名；custom / 空 → null。</summary>
    public static string? ResolveFileName(string? serviceType)
    {
        if (string.IsNullOrWhiteSpace(serviceType))
            return null;

        var key = serviceType.Trim();
        if (key.StartsWith("custom_", StringComparison.OrdinalIgnoreCase))
            return null;

        if (FileByServiceType.TryGetValue(key, out var mapped))
            return mapped;

        return key + ".svg";
    }
}
