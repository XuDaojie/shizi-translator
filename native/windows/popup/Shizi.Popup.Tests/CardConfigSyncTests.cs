using Shizi.Popup.State;
using Xunit;

namespace Shizi.Popup.Tests;

public class CardConfigSyncTests
{
    private static CardState Card(string id, CardStatus status = CardStatus.Pending) =>
        new()
        {
            ServiceInstanceId = id,
            ServiceName = id,
            ServiceType = id,
            Protocol = "openai_chat",
            ModelName = "m",
            Text = status == CardStatus.Finished ? "ok" : "",
            Status = status,
            Collapsed = true,
        };

    [Fact]
    public void SyncCards_idle_builds_cards_in_enabled_order()
    {
        var s = new PopupTranslationState();
        // 先有乱序卡
        s.SyncCards(
            new[]
            {
                new EnabledService("c", "C"),
                new EnabledService("a", "A"),
            },
            isTranslating: false);

        s.SyncCards(
            new[]
            {
                new EnabledService("a", "A"),
                new EnabledService("b", "B"),
                new EnabledService("c", "C"),
            },
            isTranslating: false);

        Assert.Equal(new[] { "a", "b", "c" }, s.CardOrder);
        Assert.Equal(CardStatus.Pending, s.Cards["b"].Status);
        Assert.Equal("B", s.Cards["b"].ServiceName);
    }

    [Fact]
    public void SyncCards_idle_removes_disabled_service_cards()
    {
        var s = new PopupTranslationState();
        s.SyncCards(
            new[]
            {
                new EnabledService("a", "A"),
                new EnabledService("b", "B"),
            },
            isTranslating: false);

        s.SyncCards(
            new[] { new EnabledService("a", "A") },
            isTranslating: false);

        Assert.Equal(new[] { "a" }, s.CardOrder);
        Assert.False(s.Cards.ContainsKey("b"));
    }

    [Fact]
    public void SyncCards_while_translating_does_not_add_unparticipating_service()
    {
        var cards = new Dictionary<string, CardState>
        {
            ["c"] = Card("c", CardStatus.Translating),
            ["a"] = Card("a", CardStatus.Translating),
        };

        CardConfigSync.SyncCards(
            cards,
            new[]
            {
                new EnabledService("a", "A-new"),
                new EnabledService("b", "B"),
                new EnabledService("c", "C-new"),
            },
            isTranslating: true);

        Assert.Equal(new[] { "a", "c" }, cards.Keys.ToArray());
        Assert.Equal("A-new", cards["a"].ServiceName);
        Assert.Equal("C-new", cards["c"].ServiceName);
        Assert.False(cards.ContainsKey("b"));
    }

    [Fact]
    public void SyncCards_while_translating_keeps_translating_disabled_card_at_end()
    {
        var cards = new Dictionary<string, CardState>
        {
            ["a"] = Card("a", CardStatus.Translating),
            ["b"] = Card("b", CardStatus.Translating),
        };

        CardConfigSync.SyncCards(
            cards,
            new[] { new EnabledService("a", "A") },
            isTranslating: true);

        Assert.Equal(new[] { "a", "b" }, cards.Keys.ToArray());
        Assert.Equal(CardStatus.Translating, cards["b"].Status);
    }

    [Fact]
    public void SyncCards_while_translating_drops_non_translating_disabled_card()
    {
        var cards = new Dictionary<string, CardState>
        {
            ["a"] = Card("a", CardStatus.Translating),
            ["b"] = Card("b", CardStatus.Finished),
        };

        CardConfigSync.SyncCards(
            cards,
            new[] { new EnabledService("a", "A") },
            isTranslating: true);

        Assert.Equal(new[] { "a" }, cards.Keys.ToArray());
    }

    [Fact]
    public void EnabledPayloads_filters_and_clears_edge_model()
    {
        var payloads = CardConfigSync.EnabledPayloads(new[]
        {
            ("a", "A", "a", "openai_chat", "gpt", true),
            ("b", "B", "b", "openai_chat", "m", false),
            ("ms", "Edge", "ms", "microsoft_edge", "gpt-x", true),
        });

        Assert.Equal(2, payloads.Count);
        Assert.Equal("a", payloads[0].ServiceInstanceId);
        Assert.Equal("gpt", payloads[0].ModelName);
        Assert.Equal("ms", payloads[1].ServiceInstanceId);
        Assert.Equal("", payloads[1].ModelName);
    }

    [Fact]
    public void PopupTranslationState_SyncCards_idle_creates_pending_cards()
    {
        var s = new PopupTranslationState();
        s.SyncCards(
            new[]
            {
                new EnabledService("svc-a", "OpenAI", "openai", "openai_chat", "gpt-4o"),
                new EnabledService("svc-b", "Claude", "claude", "claude_messages", "sonnet"),
            },
            isTranslating: false);

        Assert.Equal(2, s.Cards.Count);
        Assert.Equal(new[] { "svc-a", "svc-b" }, s.CardOrder);
        Assert.Equal("OpenAI", s.Cards["svc-a"].ServiceName);
        Assert.Equal("gpt-4o", s.Cards["svc-a"].ModelName);
    }
}
