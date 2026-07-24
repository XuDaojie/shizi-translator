using Shizi.Popup.State;
using Xunit;

namespace Shizi.Popup.Tests;

public class PopupTranslationStateTests
{
    [Fact]
    public void Started_new_batch_resets_cards_and_sets_translating()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1", "A", "openai", modelName: "gpt"));
        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "旧文本"));
        Assert.Equal("旧文本", s.Cards["s1"].Text);

        s.Dispatch(TranslationEvent.Started("b2:s1", "s1", "A", "openai", modelName: "gpt"));

        Assert.Equal("b2", s.CurrentBatchId);
        Assert.True(s.IsTranslating);
        Assert.Equal(CardStatus.Translating, s.Cards["s1"].Status);
        Assert.Equal("", s.Cards["s1"].Text);
        Assert.True(s.Cards["s1"].Collapsed);
        Assert.False(s.Cards["s1"].CollapseUserOverride);
    }

    [Fact]
    public void Delta_from_stale_batch_is_ignored()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1"));
        s.Dispatch(TranslationEvent.Delta("b2:s1", "s1", "x"));
        Assert.Equal("", s.Cards["s1"].Text);
    }

    [Fact]
    public void First_non_empty_delta_expands_without_user_override()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1", "A", "openai"));
        Assert.True(s.Cards["s1"].Collapsed);

        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "Hel"));
        Assert.False(s.Cards["s1"].Collapsed);
        Assert.Equal("Hel", s.Cards["s1"].Text);
    }

    [Fact]
    public void Empty_delta_does_not_expand()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1"));
        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", ""));
        Assert.True(s.Cards["s1"].Collapsed);
    }

    [Fact]
    public void Failed_only_affects_that_card()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1", "A", "openai"));
        s.Dispatch(TranslationEvent.Started("b1:s2", "s2", "B", "claude"));
        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "ok-so-far"));
        s.Dispatch(TranslationEvent.Failed("b1:s2", "s2", "网络错误"));

        Assert.Equal(CardStatus.Translating, s.Cards["s1"].Status);
        Assert.Equal("ok-so-far", s.Cards["s1"].Text);
        Assert.Equal(CardStatus.Failed, s.Cards["s2"].Status);
        Assert.Equal("网络错误", s.Cards["s2"].ErrorMessage);
        Assert.Equal("popup.error.translationFailed", s.Cards["s2"].ErrorTitleKey);
        Assert.False(s.Cards["s2"].ShowActions);
        Assert.Null(s.Cards["s2"].Usage);
        Assert.False(s.Cards["s2"].Collapsed);
    }

    [Fact]
    public void Finished_writes_fullText_and_usage()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1", "A", "openai"));
        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "部分"));
        s.Dispatch(TranslationEvent.Finished(
            "b1:s1",
            "s1",
            fullText: "完整译文",
            usage: new TokenUsage(10, 20),
            detectedSourceLang: "en-US"));

        var card = s.Cards["s1"];
        Assert.Equal("完整译文", card.Text);
        Assert.Equal(CardStatus.Finished, card.Status);
        Assert.NotNull(card.Usage);
        Assert.Equal(10, card.Usage!.InputTokens);
        Assert.Equal(20, card.Usage.OutputTokens);
        Assert.Equal("en-US", card.DetectedSourceLang);
        Assert.True(card.ShowActions);
        Assert.False(card.Collapsed);
    }

    [Fact]
    public void Same_batch_new_service_does_not_reset_existing()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1", "A", "openai"));
        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "保留"));
        s.Dispatch(TranslationEvent.Started("b1:s2", "s2", "B", "claude"));

        Assert.Equal("保留", s.Cards["s1"].Text);
        Assert.Equal(CardStatus.Translating, s.Cards["s2"].Status);
        Assert.Equal("b1", s.CurrentBatchId);
    }

    [Fact]
    public void Cancelled_keeps_partial_text()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1"));
        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "部分"));
        s.Dispatch(TranslationEvent.Cancelled("b1:s1", "s1"));

        Assert.Equal(CardStatus.Cancelled, s.Cards["s1"].Status);
        Assert.Equal("部分", s.Cards["s1"].Text);
        Assert.Equal("popup.status.cancelled", s.Cards["s1"].ErrorTitleKey);
    }

    [Fact]
    public void User_override_blocks_auto_expand_on_delta()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1"));
        var card = s.Cards["s1"];
        card.CollapseUserOverride = true;
        card.Collapsed = true;

        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "Hello"));
        Assert.True(card.Collapsed);
        Assert.Equal("Hello", card.Text);
    }

    [Fact]
    public void New_batch_clears_user_override()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1"));
        var card = s.Cards["s1"];
        card.CollapseUserOverride = true;
        card.Collapsed = true;

        s.Dispatch(TranslationEvent.Started("b2:s1", "s1"));
        Assert.False(card.CollapseUserOverride);
        Assert.True(card.Collapsed);
    }

    [Fact]
    public void Multi_service_only_expands_card_with_text()
    {
        var s = new PopupTranslationState();
        s.Dispatch(TranslationEvent.Started("b1:s1", "s1", "A"));
        s.Dispatch(TranslationEvent.Started("b1:s2", "s2", "B"));
        s.Dispatch(TranslationEvent.Delta("b1:s1", "s1", "仅 A"));

        Assert.False(s.Cards["s1"].Collapsed);
        Assert.True(s.Cards["s2"].Collapsed);
    }

    [Fact]
    public void BatchIdFromSession_no_colon_returns_null()
    {
        Assert.Null(SessionIds.BatchIdFromSession("no-colon"));
        Assert.Null(SessionIds.BatchIdFromSession(null));
        Assert.Equal("batch-001", SessionIds.BatchIdFromSession("batch-001:svc-a"));
    }
}
