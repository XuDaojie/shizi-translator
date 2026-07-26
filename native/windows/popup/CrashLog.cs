namespace Shizi.Popup;

/// <summary>进程崩溃/未处理异常落盘，便于排查「翻译一触发就进程消失」。</summary>
internal static class CrashLog
{
    private static readonly object Gate = new();

    public static string LogPath =>
        Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "shizi",
            "popup-crash.log");

    public static void Write(string where, Exception ex)
    {
        try
        {
            var dir = Path.GetDirectoryName(LogPath);
            if (!string.IsNullOrEmpty(dir))
                Directory.CreateDirectory(dir);

            var line =
                $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}] {where}: {ex}\n";
            lock (Gate)
            {
                File.AppendAllText(LogPath, line);
            }
        }
        catch
        {
            // best-effort
        }
    }

    public static void Write(string message)
    {
        try
        {
            var dir = Path.GetDirectoryName(LogPath);
            if (!string.IsNullOrEmpty(dir))
                Directory.CreateDirectory(dir);

            lock (Gate)
            {
                File.AppendAllText(LogPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}] {message}\n");
            }
        }
        catch
        {
            // best-effort
        }
    }
}
