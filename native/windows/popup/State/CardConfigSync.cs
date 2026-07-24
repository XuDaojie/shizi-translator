namespace Shizi.Popup.State;

/// <summary>
/// 按启用服务列表同步卡片 Map，对齐 Vue <c>syncCardsFromEnabledServices</c>。
/// </summary>
public static class CardConfigSync
{
    /// <summary>
    /// - 空闲：按启用序增删改卡片
    /// - 翻译中：不新增未参与服务卡；可删非 translating 的已禁用卡；元数据可更新；
    ///   仍在 translating 但已禁用的卡挂在末尾
    /// </summary>
    public static void SyncCards(
        IDictionary<string, CardState> cards,
        IReadOnlyList<EnabledService> enabled,
        bool isTranslating)
    {
        var next = new Dictionary<string, CardState>();

        foreach (var p in enabled)
        {
            if (cards.TryGetValue(p.ServiceInstanceId, out var existing))
            {
                ApplyMeta(existing, p);
                next[p.ServiceInstanceId] = existing;
            }
            else if (!isTranslating)
            {
                next[p.ServiceInstanceId] = CardState.CreatePending(
                    p.ServiceInstanceId,
                    p.ServiceName,
                    p.ServiceType,
                    p.Protocol,
                    p.ModelName);
            }
        }

        // 翻译中：保留仍在输出、但已从启用列表移除的卡（挂在末尾）
        if (isTranslating)
        {
            foreach (var (id, card) in cards)
            {
                if (!next.ContainsKey(id) && card.Status == CardStatus.Translating)
                    next[id] = card;
            }
        }

        cards.Clear();
        foreach (var (id, card) in next)
            cards[id] = card;
    }

    /// <summary>
    /// 从完整服务配置中提取启用项（保序）。
    /// microsoft_edge 清空 modelName，对齐 Vue <c>enabledPayloads</c>。
    /// </summary>
    public static IReadOnlyList<EnabledService> EnabledPayloads(
        IEnumerable<(string Id, string Name, string ServiceType, string Protocol, string Model, bool Enabled)> services)
    {
        var list = new List<EnabledService>();
        foreach (var s in services)
        {
            if (!s.Enabled)
                continue;

            var modelName = s.Protocol == "microsoft_edge" ? "" : s.Model;
            list.Add(new EnabledService(
                s.Id,
                s.Name,
                s.ServiceType,
                s.Protocol,
                modelName));
        }

        return list;
    }

    private static void ApplyMeta(CardState card, EnabledService p)
    {
        card.ServiceName = p.ServiceName;
        card.ServiceType = p.ServiceType;
        card.Protocol = p.Protocol;
        card.ModelName = p.ModelName;
    }
}
