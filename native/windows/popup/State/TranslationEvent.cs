namespace Shizi.Popup.State;

public enum TranslationEventType
{
    Started,
    Delta,
    Finished,
    Failed,
    Cancelled,
}

/// <summary>翻译事件载荷，对齐 Vue <c>TranslationEventPayload</c>。</summary>
public sealed class TranslationEvent
{
    public TranslationEventType Type { get; init; }
    public string? SessionId { get; init; }
    public string? ServiceInstanceId { get; init; }
    public string? ServiceName { get; init; }
    public string? ServiceType { get; init; }
    public string? Protocol { get; init; }
    public string? ModelName { get; init; }
    public string? Text { get; init; }
    public string? FullText { get; init; }
    public string? Message { get; init; }
    public string? DetectedSourceLang { get; init; }
    public TokenUsage? Usage { get; init; }

    public static TranslationEvent Started(
        string sessionId,
        string serviceInstanceId,
        string? serviceName = null,
        string? serviceType = null,
        string? protocol = null,
        string? modelName = null) =>
        new()
        {
            Type = TranslationEventType.Started,
            SessionId = sessionId,
            ServiceInstanceId = serviceInstanceId,
            ServiceName = serviceName,
            ServiceType = serviceType,
            Protocol = protocol,
            ModelName = modelName,
        };

    public static TranslationEvent Delta(string sessionId, string serviceInstanceId, string text) =>
        new()
        {
            Type = TranslationEventType.Delta,
            SessionId = sessionId,
            ServiceInstanceId = serviceInstanceId,
            Text = text,
        };

    public static TranslationEvent Finished(
        string sessionId,
        string serviceInstanceId,
        string? fullText = null,
        TokenUsage? usage = null,
        string? detectedSourceLang = null) =>
        new()
        {
            Type = TranslationEventType.Finished,
            SessionId = sessionId,
            ServiceInstanceId = serviceInstanceId,
            FullText = fullText,
            Usage = usage,
            DetectedSourceLang = detectedSourceLang,
        };

    public static TranslationEvent Failed(string sessionId, string serviceInstanceId, string? message = null) =>
        new()
        {
            Type = TranslationEventType.Failed,
            SessionId = sessionId,
            ServiceInstanceId = serviceInstanceId,
            Message = message,
        };

    public static TranslationEvent Cancelled(string sessionId, string serviceInstanceId) =>
        new()
        {
            Type = TranslationEventType.Cancelled,
            SessionId = sessionId,
            ServiceInstanceId = serviceInstanceId,
        };
}
