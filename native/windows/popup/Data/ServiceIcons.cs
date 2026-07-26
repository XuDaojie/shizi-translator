using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.UI;

namespace Shizi.Popup.Data;

/// <summary>
/// WinUI 侧加载设置页同源服务图标（Assets/service-icons）。
/// 映射表见 <see cref="ServiceIconMap"/>。
/// </summary>
public static class ServiceIcons
{
    private static string IconsDir =>
        Path.Combine(AppContext.BaseDirectory, "Assets", "service-icons");

    /// <summary>14×14 结果卡服务图标；无资源时首字色块兜底。</summary>
    public static UIElement Create(string? serviceType, string displayName, double size = 14)
    {
        var path = ResolveIconPath(serviceType);
        if (path is not null)
        {
            try
            {
                var image = new Image
                {
                    Width = size,
                    Height = size,
                    Stretch = Stretch.Uniform,
                    VerticalAlignment = VerticalAlignment.Center,
                    HorizontalAlignment = HorizontalAlignment.Center,
                };
                image.Source = new SvgImageSource(new Uri(path));
                return image;
            }
            catch
            {
                // fall through
            }
        }

        return CreateLetterFallback(displayName, size);
    }

    public static string? ResolveIconPath(string? serviceType)
    {
        var fileName = ServiceIconMap.ResolveFileName(serviceType);
        if (fileName is null)
            return null;

        var full = Path.Combine(IconsDir, fileName);
        if (File.Exists(full))
            return full;

        var key = serviceType!.Trim();
        var alt = Path.Combine(IconsDir, key.ToLowerInvariant() + ".svg");
        return File.Exists(alt) ? alt : null;
    }

    private static UIElement CreateLetterFallback(string displayName, double size)
    {
        var initial = string.IsNullOrWhiteSpace(displayName)
            ? "?"
            : char.ToUpperInvariant(displayName.Trim()[0]).ToString();
        var accent = Color.FromArgb(255, 0xD5, 0x5A, 0x1F);
        return new Border
        {
            Width = size,
            Height = size,
            CornerRadius = new CornerRadius(3),
            Background = new SolidColorBrush(accent),
            VerticalAlignment = VerticalAlignment.Center,
            Child = new TextBlock
            {
                Text = initial,
                FontSize = Math.Max(8, size * 0.65),
                FontWeight = Microsoft.UI.Text.FontWeights.Bold,
                Foreground = new SolidColorBrush(Colors.White),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };
    }
}
