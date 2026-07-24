using System.Text.Json;
using Shizi.Popup.Host;

namespace Shizi.Popup.Bridge;

/// <summary>
/// Bridge 层：Rust→UI push 与 UI→Rust request。
/// 完整状态机与 UI 区块留给任务 9–10；此处做最小接线与占位反馈。
/// </summary>
public sealed class NativeBridge
{
    public static NativeBridge Instance { get; } = new();

    private PopupController? _controller;
    private Action<string>? _requestSink;

    private NativeBridge()
    {
    }

    public void Attach(PopupController controller)
    {
        _controller = controller;
    }

    public void SetRequestSink(Action<string> sink)
    {
        _requestSink = sink;
    }

    /// <summary>Rust → UI：接收 BridgePush JSON 字符串。</summary>
    public void ReceivePushJson(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            var typeName = root.TryGetProperty("type", out var t) ? t.GetString() ?? "?" : "?";
            _controller?.Window.OnBridgePushHint(typeName);

            // 任务 9：在此接入 TranslationEventReducer
            // 任务 10：在此驱动完整 UI 绑定
        }
        catch
        {
            _controller?.Window.OnBridgePushHint("push?");
        }
    }

    /// <summary>
    /// UI → Rust：发送 BridgeEnvelope JSON（任务 10 控件会调用）。
    /// 例：start_translation / ready / get_app_config。
    /// </summary>
    public void SendRequest(string envelopeJson)
    {
        _requestSink?.Invoke(envelopeJson);
    }

    /// <summary>便捷：构造标准 envelope 并发送。</summary>
    public void SendRequest(string typeName, object? payload = null, string? requestId = null)
    {
        var body = new Dictionary<string, object?>
        {
            ["bridgeVersion"] = 1,
            ["type"] = typeName,
        };
        if (requestId is not null)
        {
            body["requestId"] = requestId;
        }
        if (payload is not null)
        {
            body["payload"] = payload;
        }

        var json = JsonSerializer.Serialize(body);
        SendRequest(json);
    }

    /// <summary>UI 首帧可交互时调用（对齐 readyGate）。</summary>
    public void NotifyReady()
    {
        SendRequest("ready", requestId: Guid.NewGuid().ToString("N"));
    }
}
