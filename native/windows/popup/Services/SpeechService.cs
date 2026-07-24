using System.Runtime.InteropServices.WindowsRuntime;
using Windows.Media.Core;
using Windows.Media.Playback;
using Windows.Media.SpeechSynthesis;

namespace Shizi.Popup.Services;

/// <summary>系统 TTS 朗读（Windows.Media.SpeechSynthesis）。</summary>
public sealed class SpeechService : IDisposable
{
    private readonly SpeechSynthesizer _synth = new();
    private readonly MediaPlayer _player = new();
    private bool _disposed;

    public async Task SpeakAsync(string text, CancellationToken ct = default)
    {
        if (string.IsNullOrWhiteSpace(text) || _disposed)
            return;

        try
        {
            _player.Pause();
            // IAsyncOperation → Task（WinRT interop）
            var op = _synth.SynthesizeTextToStreamAsync(text);
            var stream = await op.AsTask(ct).ConfigureAwait(true);
            _player.Source = MediaSource.CreateFromStream(stream, stream.ContentType);
            _player.Play();
        }
        catch
        {
            // best-effort：无语音包或 COM 失败时静默
        }
    }

    public void Stop()
    {
        try
        {
            _player.Pause();
        }
        catch
        {
            // ignore
        }
    }

    public void Dispose()
    {
        if (_disposed)
            return;
        _disposed = true;
        try
        {
            _player.Dispose();
        }
        catch
        {
            // ignore
        }

        try
        {
            _synth.Dispose();
        }
        catch
        {
            // ignore
        }
    }
}
