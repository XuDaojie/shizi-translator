namespace Shizi.Popup.State;

/// <summary>
/// 弹窗翻译事件状态机（纯逻辑），对齐 Vue <c>useTranslationEvents</c> 的 dispatch 语义。
/// </summary>
public sealed class PopupTranslationState
{
    private readonly Dictionary<string, CardState> _cards = new();

    public IReadOnlyDictionary<string, CardState> Cards => _cards;

    /// <summary>卡片保序键列表（与 Map 插入序一致）。</summary>
    public IReadOnlyList<string> CardOrder => _cards.Keys.ToList();

    public string? CurrentBatchId { get; private set; }

    public bool IsTranslating { get; private set; }

    public void Dispatch(TranslationEvent ev)
    {
        switch (ev.Type)
        {
            case TranslationEventType.Started:
                OnStarted(ev);
                break;
            case TranslationEventType.Delta:
                OnDelta(ev);
                break;
            case TranslationEventType.Finished:
                OnFinished(ev);
                break;
            case TranslationEventType.Failed:
                OnFailed(ev);
                break;
            case TranslationEventType.Cancelled:
                OnCancelled(ev);
                break;
        }
    }

    /// <summary>
    /// 同步卡片配置。
    /// <paramref name="isTranslating"/> 可由调用方覆盖（与 Vue 父组件传 options 一致）；
    /// 若需跟内部标志，可传 <see cref="IsTranslating"/>。
    /// </summary>
    public void SyncCards(IReadOnlyList<EnabledService> enabled, bool isTranslating)
    {
        CardConfigSync.SyncCards(_cards, enabled, isTranslating);
    }

    private void OnStarted(TranslationEvent ev)
    {
        var batchId = SessionIds.BatchIdFromSession(ev.SessionId);
        var isNewBatch = batchId != CurrentBatchId;
        if (isNewBatch)
        {
            CurrentBatchId = batchId;
            foreach (var existing in _cards.Values)
                existing.ResetForNewBatch();
            IsTranslating = true;
        }

        var card = EnsureCard(ev);
        if (ev.ServiceName is not null)
            card.ServiceName = ev.ServiceName;
        if (ev.ServiceType is not null)
            card.ServiceType = ev.ServiceType;
        if (ev.Protocol is not null)
            card.Protocol = ev.Protocol;
        if (ev.ModelName is not null)
            card.ModelName = ev.ModelName;

        card.Status = CardStatus.Translating;
        card.Text = "";
        card.ShowActions = false;
        card.Usage = null;
        card.Expanded = false;
        card.HasOverflow = false;
        card.DetectedSourceLang = null;
        card.ErrorTitleKey = null;
        card.ErrorMessage = "";
        if (!card.CollapseUserOverride)
            card.Collapsed = true;
    }

    private void OnDelta(TranslationEvent ev)
    {
        if (SessionIds.BatchIdFromSession(ev.SessionId) != CurrentBatchId)
            return;

        if (!_cards.TryGetValue(ServiceId(ev), out var card))
            return;

        var prevLen = card.Text.Length;
        card.Text += ev.Text ?? "";
        if (!card.CollapseUserOverride && prevLen == 0 && card.Text.Length > 0)
            card.Collapsed = false;
    }

    private void OnFinished(TranslationEvent ev)
    {
        if (SessionIds.BatchIdFromSession(ev.SessionId) != CurrentBatchId)
            return;

        if (!_cards.TryGetValue(ServiceId(ev), out var card))
            return;

        card.Text = ev.FullText ?? card.Text;
        card.Status = CardStatus.Finished;
        card.Usage = ev.Usage;
        card.ShowActions = true;
        card.DetectedSourceLang = ev.DetectedSourceLang;
        if (!card.CollapseUserOverride && card.Text.Trim().Length > 0)
            card.Collapsed = false;
    }

    private void OnFailed(TranslationEvent ev)
    {
        if (SessionIds.BatchIdFromSession(ev.SessionId) != CurrentBatchId)
            return;

        if (!_cards.TryGetValue(ServiceId(ev), out var card))
            return;

        card.ErrorMessage = ev.Message ?? "";
        card.ErrorTitleKey = "popup.error.translationFailed";
        card.Status = CardStatus.Failed;
        card.ShowActions = false;
        card.Usage = null;
        if (!card.CollapseUserOverride)
            card.Collapsed = false;
    }

    private void OnCancelled(TranslationEvent ev)
    {
        if (SessionIds.BatchIdFromSession(ev.SessionId) != CurrentBatchId)
            return;

        if (!_cards.TryGetValue(ServiceId(ev), out var card))
            return;

        card.ErrorTitleKey = "popup.status.cancelled";
        card.Status = CardStatus.Cancelled;
    }

    private CardState EnsureCard(TranslationEvent ev)
    {
        var id = ServiceId(ev);
        if (_cards.TryGetValue(id, out var card))
            return card;

        card = CardState.CreatePending(
            id,
            ev.ServiceName ?? "",
            ev.ServiceType ?? "",
            ev.Protocol ?? "",
            ev.ModelName ?? "");
        _cards[id] = card;
        return card;
    }

    private static string ServiceId(TranslationEvent ev) =>
        ev.ServiceInstanceId ?? "default";
}
