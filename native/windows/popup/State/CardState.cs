namespace Shizi.Popup.State;

/// <summary>结果卡状态，对齐 Vue <c>CardState</c>。</summary>
public sealed class CardState
{
    public string ServiceInstanceId { get; set; } = "";
    public string ServiceName { get; set; } = "";
    public string ServiceType { get; set; } = "";
    /// <summary>协议 id（openai_chat / claude_messages / microsoft_edge）。</summary>
    public string Protocol { get; set; } = "";
    public string ModelName { get; set; } = "";
    public string Text { get; set; } = "";
    public CardStatus Status { get; set; } = CardStatus.Pending;
    public bool Collapsed { get; set; } = true;
    /// <summary>用户在本 batch 内手动改过折叠；true 时自动规则不改 collapsed，新 batch 清除。</summary>
    public bool CollapseUserOverride { get; set; }
    public bool Expanded { get; set; }
    public bool HasOverflow { get; set; }
    public bool ShowActions { get; set; }
    public TokenUsage? Usage { get; set; }
    public string? DetectedSourceLang { get; set; }
    public string? ErrorTitleKey { get; set; }
    public string ErrorMessage { get; set; } = "";

    public static CardState CreatePending(
        string serviceInstanceId,
        string serviceName = "",
        string serviceType = "",
        string protocol = "",
        string modelName = "")
    {
        return new CardState
        {
            ServiceInstanceId = serviceInstanceId,
            ServiceName = serviceName,
            ServiceType = serviceType,
            Protocol = protocol,
            ModelName = modelName,
            Text = "",
            Status = CardStatus.Pending,
            Collapsed = true,
            CollapseUserOverride = false,
            Expanded = false,
            HasOverflow = false,
            ShowActions = false,
            Usage = null,
            DetectedSourceLang = null,
            ErrorTitleKey = null,
            ErrorMessage = "",
        };
    }

    internal void ResetForNewBatch()
    {
        Status = CardStatus.Pending;
        Text = "";
        ShowActions = false;
        Usage = null;
        Expanded = false;
        HasOverflow = false;
        DetectedSourceLang = null;
        ErrorTitleKey = null;
        ErrorMessage = "";
        Collapsed = true;
        CollapseUserOverride = false;
    }
}

public enum CardStatus
{
    Pending,
    Translating,
    Finished,
    Failed,
    Cancelled,
}

public sealed class TokenUsage
{
    public TokenUsage(int inputTokens, int outputTokens)
    {
        InputTokens = inputTokens;
        OutputTokens = outputTokens;
    }

    public int InputTokens { get; }
    public int OutputTokens { get; }
}
