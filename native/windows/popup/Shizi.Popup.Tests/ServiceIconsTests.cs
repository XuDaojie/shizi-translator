using Shizi.Popup.Data;
using Xunit;

namespace Shizi.Popup.Tests;

public class ServiceIconsTests
{
    [Theory]
    [InlineData("openai", "openai.svg")]
    [InlineData("deepseek", "deepseek.svg")]
    [InlineData("claude", "claude.svg")]
    [InlineData("microsoft", "microsoft.svg")]
    [InlineData("gemini", "google.svg")]
    [InlineData("google", "googletranslate.svg")]
    [InlineData("siliconflow", "siliconflow.svg")]
    [InlineData("volcengine", "volcengine.svg")]
    [InlineData("zhipu", "zhipu.svg")]
    [InlineData("moonshot", "moonshotai.svg")]
    [InlineData("OpenAI", "openai.svg")]
    public void ResolveFileName_aligns_with_settings_service_type(string type, string fileName)
    {
        Assert.Equal(fileName, ServiceIconMap.ResolveFileName(type));
    }

    [Fact]
    public void ResolveFileName_returns_null_for_custom_and_empty()
    {
        Assert.Null(ServiceIconMap.ResolveFileName("custom_abc"));
        Assert.Null(ServiceIconMap.ResolveFileName(""));
        Assert.Null(ServiceIconMap.ResolveFileName(null));
    }
}
