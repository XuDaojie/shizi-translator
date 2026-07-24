using Shizi.Popup.Data;
using Xunit;

namespace Shizi.Popup.Tests;

public class TranslationLanguagesTests
{
    [Fact]
    public void Target_language_codes_match_expected_count()
    {
        Assert.Equal(19, TranslationLanguages.Targets.Count);
        Assert.Contains(TranslationLanguages.All, l => l.Code == "auto");
    }

    [Fact]
    public void All_contains_auto_plus_targets()
    {
        Assert.Equal(20, TranslationLanguages.All.Count);
        Assert.Equal("auto", TranslationLanguages.All[0].Code);
    }

    [Fact]
    public void Expected_target_codes_are_present()
    {
        string[] expected =
        [
            "zh-CN", "zh-TW", "en", "ja", "ko", "fr", "de", "es", "pt", "ru",
            "it", "nl", "pl", "tr", "ar", "th", "vi", "id", "hi",
        ];

        Assert.Equal(expected, TranslationLanguages.Targets.Select(t => t.Code).ToArray());
    }
}
