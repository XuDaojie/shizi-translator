using System.Collections.Concurrent;
using System.Text.Json;
using Shizi.Popup.Bridge;
using Shizi.Popup.State;

namespace Shizi.Popup.Services;

/// <summary>
/// 翻译弹窗业务桥：解析 Rust push、匹配 request 响应、维护 <see cref="PopupTranslationState"/>。
/// UI 线程通过 <see cref="UiChanged"/> 刷新绑定。
/// </summary>
public sealed class BridgeService
{
    public static BridgeService Instance { get; } = new();

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly ConcurrentDictionary<string, TaskCompletionSource<BridgeResponseDto>> _pending = new();
    private readonly object _gate = new();
    private AppConfigDto? _pendingConfigRefresh;
    private int _sourceRevision;

    private BridgeService()
    {
        State = new PopupTranslationState();
        SourceText = "";
        SessionSourceLang = "auto";
        SessionTargetLang = "zh-CN";
        StatusKey = "popup.status.ready";
        StatusLoading = false;
    }

    public PopupTranslationState State { get; }

    /// <summary>UI 原文（可编辑）。</summary>
    public string SourceText { get; set; }

    public string SessionSourceLang { get; set; }
    public string SessionTargetLang { get; set; }

    /// <summary>selectedText / ocrText / manualText / null。</summary>
    public string? SourceBadge { get; private set; }

    public string DetectedLangBadge { get; private set; } = "";

    /// <summary>与 Vue isTranslating 对齐的 UI 层标志（State 仅在 started 时置 true）。</summary>
    public bool IsTranslating { get; private set; }

    public string StatusKey { get; private set; }
    public bool StatusLoading { get; private set; }
    public StatusActionKind StatusAction { get; private set; } = StatusActionKind.None;

    public string TipMessage { get; private set; } = "";

    /// <summary>任意 UI 可见状态变更（已保证可在任意线程触发；UI 侧需 Dispatcher）。</summary>
    public event Action? UiChanged;

    public event Action? LocaleChanged;

    /// <summary>Rust → UI：解析 BridgePush / BridgeResponse JSON。</summary>
    public void HandlePushJson(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            var typeName = root.TryGetProperty("type", out var t) ? t.GetString() ?? "" : "";

            switch (typeName)
            {
                case "translation_event":
                    HandleTranslationEvent(GetPayload(root));
                    break;
                case "app_config_changed":
                    HandleAppConfigChanged(GetPayload(root));
                    break;
                case "interface_language_changed":
                    HandleInterfaceLanguageChanged(GetPayload(root));
                    break;
                case "show_context":
                    HandleShowContext(GetPayload(root));
                    break;
                case "response":
                    HandleResponse(root);
                    break;
            }
        }
        catch
        {
            // 坏 JSON 忽略
        }
    }

    /// <summary>冷启动流水线：ready → config → pending 原文（可触发翻译）。</summary>
    public async Task RunStartupSequenceAsync()
    {
        // 无 IPC 时仅本地壳，跳过 request 等待
        if (!NativeBridge.Instance.HasRequestSink)
        {
            RaiseUi();
            return;
        }

        NativeBridge.Instance.NotifyReady();

        try
        {
            var config = await GetAppConfigAsync().ConfigureAwait(true);
            if (config is not null)
            {
                ApplyConfig(config, forceSync: true);
            }
        }
        catch
        {
            // best-effort
        }

        try
        {
            var pending = await TakePendingSourceTextAsync().ConfigureAwait(true);
            var text = pending?.Trim();
            if (!string.IsNullOrEmpty(text) && !IsTranslating)
            {
                _sourceRevision++;
                SourceText = text;
                RaiseUi();
                StartTranslation(text);
            }
        }
        catch
        {
            ShowTip(Localization.T("popup.error.pendingSourceFailed"));
        }
    }

    public void StartTranslation(string? text = null)
    {
        var body = (text ?? SourceText).Trim();
        if (string.IsNullOrEmpty(body))
        {
            ShowTip(Localization.T("popup.error.emptySource"));
            return;
        }

        if (IsTranslating)
            return;

        SourceText = body;
        _sourceRevision++;
        RaiseUi();

        NativeBridge.Instance.SendRequest(
            "start_translation",
            new Dictionary<string, object?> { ["text"] = body },
            Guid.NewGuid().ToString("N"));
    }

    public void CancelTranslation()
    {
        NativeBridge.Instance.SendRequest(
            "cancel_translation",
            requestId: Guid.NewGuid().ToString("N"));
    }

    public void RetryTranslation()
    {
        if (IsTranslating)
            return;

        NativeBridge.Instance.SendRequest(
            "retry_translation",
            requestId: Guid.NewGuid().ToString("N"));
    }

    public void SetSessionLanguages(string sourceLang, string targetLang)
    {
        SessionSourceLang = sourceLang;
        SessionTargetLang = targetLang;
        DetectedLangBadge = "";
        RaiseUi();

        NativeBridge.Instance.SendRequest(
            "set_session_languages",
            new Dictionary<string, object?>
            {
                ["sourceLang"] = sourceLang,
                ["targetLang"] = targetLang,
            },
            Guid.NewGuid().ToString("N"));
    }

    public bool TrySwapLanguages()
    {
        if (SessionSourceLang == "auto" || SessionTargetLang == "auto")
        {
            ShowTip(Localization.T("popup.error.swapAuto"));
            return false;
        }

        var tmp = SessionSourceLang;
        SessionSourceLang = SessionTargetLang;
        SessionTargetLang = tmp;
        SetSessionLanguages(SessionSourceLang, SessionTargetLang);
        return true;
    }

    public void OpenSettings()
    {
        NativeBridge.Instance.SendRequest(
            "open_settings",
            requestId: Guid.NewGuid().ToString("N"));
    }

    public void TriggerOcrTranslation()
    {
        NativeBridge.Instance.SendRequest(
            "trigger_ocr_translation",
            requestId: Guid.NewGuid().ToString("N"));
    }

    public void ReportContentSize(double width, double height)
    {
        NativeBridge.Instance.SendRequest(
            "report_content_size",
            new Dictionary<string, object?>
            {
                ["width"] = width,
                ["height"] = height,
            });
    }

    public void NotifySourceEdited(string text)
    {
        _sourceRevision++;
        SourceText = text;
        if (string.IsNullOrWhiteSpace(text))
        {
            foreach (var card in State.Cards.Values)
            {
                card.Collapsed = true;
                card.CollapseUserOverride = false;
            }

            RaiseUi();
        }
    }

    public void ToggleCardCollapsed(string serviceInstanceId)
    {
        if (!State.Cards.TryGetValue(serviceInstanceId, out var card))
            return;

        card.Collapsed = !card.Collapsed;
        card.CollapseUserOverride = true;
        RaiseUi();
    }

    /// <summary>结果卡「展开全文 / 收起」（仅 UI 本地状态，无 Bridge 协议）。</summary>
    public void ToggleCardExpanded(string serviceInstanceId)
    {
        if (!State.Cards.TryGetValue(serviceInstanceId, out var card))
            return;

        card.Expanded = !card.Expanded;
        RaiseUi();
    }

    public void ShowTip(string message)
    {
        TipMessage = message;
        RaiseUi();
    }

    public void ClearTip()
    {
        if (string.IsNullOrEmpty(TipMessage))
            return;
        TipMessage = "";
        RaiseUi();
    }

    public async Task<AppConfigDto?> GetAppConfigAsync(int timeoutMs = 8000)
    {
        var resp = await RequestAsync("get_app_config", null, timeoutMs).ConfigureAwait(true);
        if (resp is null || !resp.Ok || resp.Body is null || resp.Body.Value.ValueKind is JsonValueKind.Null or JsonValueKind.Undefined)
            return null;

        try
        {
            return JsonSerializer.Deserialize<AppConfigDto>(resp.Body.Value.GetRawText(), JsonOptions);
        }
        catch
        {
            return null;
        }
    }

    public async Task<string?> TakePendingSourceTextAsync(int timeoutMs = 5000)
    {
        var resp = await RequestAsync("take_pending_source_text", null, timeoutMs).ConfigureAwait(true);
        if (resp is null || !resp.Ok || resp.Body is null)
            return null;

        var body = resp.Body.Value;
        if (body.ValueKind == JsonValueKind.Null || body.ValueKind == JsonValueKind.Undefined)
            return null;
        if (body.ValueKind == JsonValueKind.String)
            return body.GetString();
        return body.ToString();
    }

    private async Task<BridgeResponseDto?> RequestAsync(string typeName, object? payload, int timeoutMs)
    {
        var id = Guid.NewGuid().ToString("N");
        var tcs = new TaskCompletionSource<BridgeResponseDto>(TaskCreationOptions.RunContinuationsAsynchronously);
        _pending[id] = tcs;

        NativeBridge.Instance.SendRequest(typeName, payload, id);

        using var cts = new CancellationTokenSource(timeoutMs);
        await using var reg = cts.Token.Register(() =>
        {
            if (_pending.TryRemove(id, out var pending))
                pending.TrySetResult(new BridgeResponseDto { Ok = false, Error = "timeout" });
        });

        return await tcs.Task.ConfigureAwait(true);
    }

    private void HandleResponse(JsonElement root)
    {
        var requestId = root.TryGetProperty("requestId", out var rid) ? rid.GetString() : null;
        if (string.IsNullOrEmpty(requestId))
            return;

        if (!_pending.TryRemove(requestId, out var tcs))
            return;

        var ok = root.TryGetProperty("ok", out var okEl) && okEl.ValueKind == JsonValueKind.True;
        JsonElement? body = null;
        if (root.TryGetProperty("body", out var bodyEl))
            body = bodyEl.Clone();

        var error = root.TryGetProperty("error", out var errEl) ? errEl.GetString() : null;
        tcs.TrySetResult(new BridgeResponseDto
        {
            Ok = ok,
            Body = body,
            Error = error,
        });
    }

    private void HandleTranslationEvent(JsonElement? payload)
    {
        if (payload is null || payload.Value.ValueKind != JsonValueKind.Object)
            return;

        var p = payload.Value;
        var typeStr = p.TryGetProperty("type", out var te) ? te.GetString() ?? "" : "";
        var sessionId = GetString(p, "sessionId");
        var serviceId = GetString(p, "serviceInstanceId") ?? "default";

        var beforeBatch = State.CurrentBatchId;
        TranslationEvent? ev = typeStr switch
        {
            "started" => TranslationEvent.Started(
                sessionId ?? "",
                serviceId,
                GetString(p, "serviceName"),
                GetString(p, "serviceType"),
                GetString(p, "protocol"),
                GetString(p, "modelName")),
            "delta" => TranslationEvent.Delta(sessionId ?? "", serviceId, GetString(p, "text") ?? ""),
            "finished" => TranslationEvent.Finished(
                sessionId ?? "",
                serviceId,
                GetString(p, "fullText"),
                ParseUsage(p),
                GetString(p, "detectedSourceLang")),
            "failed" => TranslationEvent.Failed(sessionId ?? "", serviceId, GetString(p, "message")),
            "cancelled" => TranslationEvent.Cancelled(sessionId ?? "", serviceId),
            _ => null,
        };

        if (ev is null)
            return;

        lock (_gate)
        {
            if (ev.Type == TranslationEventType.Started)
            {
                var isNewBatch = SessionIds.BatchIdFromSession(sessionId) != beforeBatch;
                if (isNewBatch)
                {
                    IsTranslating = true;
                    var sourceText = GetString(p, "sourceText");
                    if (!string.IsNullOrEmpty(sourceText))
                    {
                        _sourceRevision++;
                        SourceText = sourceText;
                    }

                    SourceBadge = NormalizeBadge(GetString(p, "sourceType"));
                    DetectedLangBadge = "";
                    StatusKey = "popup.status.translating";
                    StatusLoading = true;
                    StatusAction = StatusActionKind.Cancel;
                }
            }

            State.Dispatch(ev);

            if (ev.Type is TranslationEventType.Finished or TranslationEventType.Failed or TranslationEventType.Cancelled)
            {
                if (ev.Type == TranslationEventType.Finished
                    && !string.IsNullOrEmpty(ev.DetectedSourceLang)
                    && SessionSourceLang == "auto")
                {
                    DetectedLangBadge = ev.DetectedSourceLang!;
                }

                UpdateBatchStatus();
            }
        }

        RaiseUi();
    }

    private void HandleAppConfigChanged(JsonElement? payload)
    {
        if (payload is null || payload.Value.ValueKind != JsonValueKind.Object)
            return;

        try
        {
            var config = JsonSerializer.Deserialize<AppConfigDto>(payload.Value.GetRawText(), JsonOptions);
            if (config is not null)
                ApplyConfig(config, forceSync: false);
        }
        catch
        {
            // ignore
        }
    }

    private void HandleInterfaceLanguageChanged(JsonElement? payload)
    {
        string? locale = null;
        if (payload is { ValueKind: JsonValueKind.String } pe)
            locale = pe.GetString();
        else if (payload is { ValueKind: JsonValueKind.Object } po)
            locale = GetString(po, "locale") ?? GetString(po, "interfaceLanguage");

        Localization.SetLocale(locale);
        LocaleChanged?.Invoke();
        RaiseUi();
    }

    private void HandleShowContext(JsonElement? payload)
    {
        if (payload is null || payload.Value.ValueKind != JsonValueKind.Object)
            return;

        var p = payload.Value;
        var sourceText = GetString(p, "sourceText");
        var badge = NormalizeBadge(GetString(p, "sourceBadge"));

        lock (_gate)
        {
            if (!string.IsNullOrEmpty(sourceText))
            {
                _sourceRevision++;
                SourceText = sourceText;
            }

            if (badge is not null)
                SourceBadge = badge;
        }

        RaiseUi();

        // 有原文且空闲时可触发翻译（与 Vue pending 补触发类似）
        if (!string.IsNullOrWhiteSpace(sourceText) && !IsTranslating)
        {
            StartTranslation(sourceText);
        }
    }

    private void ApplyConfig(AppConfigDto config, bool forceSync)
    {
        if (!string.IsNullOrWhiteSpace(config.InterfaceLanguage))
            Localization.SetLocale(config.InterfaceLanguage);

        if (!string.IsNullOrWhiteSpace(config.DefaultSourceLang))
            SessionSourceLang = config.DefaultSourceLang!;
        if (!string.IsNullOrWhiteSpace(config.TargetLang))
            SessionTargetLang = config.TargetLang!;

        var enabled = ExtractEnabled(config);
        lock (_gate)
        {
            if (IsTranslating && !forceSync)
            {
                _pendingConfigRefresh = config;
                State.SyncCards(enabled, isTranslating: true);
            }
            else
            {
                _pendingConfigRefresh = null;
                State.SyncCards(enabled, isTranslating: IsTranslating);
            }
        }

        LocaleChanged?.Invoke();
        RaiseUi();
    }

    private void UpdateBatchStatus()
    {
        var list = State.Cards.Values.ToList();
        if (list.Count == 0)
            return;

        var allFinished = list.All(c => c.Status == CardStatus.Finished);
        var allFailed = list.All(c => c.Status is CardStatus.Failed or CardStatus.Cancelled);
        var anyTranslating = list.Any(c => c.Status == CardStatus.Translating);

        if (allFinished)
        {
            IsTranslating = false;
            SourceBadge = null;
            if (SessionSourceLang == "auto")
            {
                var detected = list.FirstOrDefault(c => !string.IsNullOrEmpty(c.DetectedSourceLang))
                    ?.DetectedSourceLang ?? "";
                DetectedLangBadge = detected;
            }

            StatusKey = "popup.status.completed";
            StatusLoading = false;
            StatusAction = StatusActionKind.Retry;
            ApplyPendingConfigRefresh();
        }
        else if (allFailed)
        {
            IsTranslating = false;
            DetectedLangBadge = "";
            StatusKey = "popup.status.failed";
            StatusLoading = false;
            StatusAction = StatusActionKind.Retry;
            ApplyPendingConfigRefresh();
        }
        else if (anyTranslating)
        {
            StatusKey = "popup.status.translating";
            StatusLoading = true;
            StatusAction = StatusActionKind.Cancel;
        }
        else
        {
            IsTranslating = false;
            SourceBadge = null;
            DetectedLangBadge = "";
            StatusKey = "popup.status.partial";
            StatusLoading = false;
            StatusAction = StatusActionKind.Retry;
            ApplyPendingConfigRefresh();
        }
    }

    private void ApplyPendingConfigRefresh()
    {
        if (_pendingConfigRefresh is null)
            return;
        var cfg = _pendingConfigRefresh;
        _pendingConfigRefresh = null;
        var enabled = ExtractEnabled(cfg);
        State.SyncCards(enabled, isTranslating: false);
    }

    private static IReadOnlyList<EnabledService> ExtractEnabled(AppConfigDto config)
    {
        if (config.Services is null || config.Services.Count == 0)
            return Array.Empty<EnabledService>();

        return CardConfigSync.EnabledPayloads(
            config.Services.Select(s => (
                s.Id ?? "",
                s.Name ?? "",
                s.ServiceType ?? "",
                s.Protocol ?? "",
                s.Model ?? "",
                s.Enabled)));
    }

    private static JsonElement? GetPayload(JsonElement root)
    {
        if (!root.TryGetProperty("payload", out var p))
            return null;
        return p.Clone();
    }

    private static string? GetString(JsonElement el, string name) =>
        el.TryGetProperty(name, out var p) && p.ValueKind == JsonValueKind.String
            ? p.GetString()
            : null;

    private static TokenUsage? ParseUsage(JsonElement p)
    {
        if (!p.TryGetProperty("usage", out var u) || u.ValueKind != JsonValueKind.Object)
            return null;

        var input = u.TryGetProperty("inputTokens", out var i) && i.TryGetInt32(out var iv) ? iv : 0;
        var output = u.TryGetProperty("outputTokens", out var o) && o.TryGetInt32(out var ov) ? ov : 0;
        return new TokenUsage(input, output);
    }

    private static string? NormalizeBadge(string? badge) =>
        badge switch
        {
            "selectedText" or "ocrText" or "manualText" => badge,
            _ => null,
        };

    private void RaiseUi() => UiChanged?.Invoke();
}

public enum StatusActionKind
{
    None,
    Cancel,
    Retry,
}

internal sealed class BridgeResponseDto
{
    public bool Ok { get; init; }
    public JsonElement? Body { get; init; }
    public string? Error { get; init; }
}

/// <summary>AppConfig 脱敏子集（仅弹窗所需字段）。</summary>
public sealed class AppConfigDto
{
    public string? InterfaceLanguage { get; set; }
    public string? TargetLang { get; set; }
    public string? DefaultSourceLang { get; set; }
    public List<ServiceInstanceDto>? Services { get; set; }
}

public sealed class ServiceInstanceDto
{
    public string? Id { get; set; }
    public string? Name { get; set; }
    public string? ServiceType { get; set; }
    public string? Protocol { get; set; }
    public string? Model { get; set; }
    public bool Enabled { get; set; }
}
