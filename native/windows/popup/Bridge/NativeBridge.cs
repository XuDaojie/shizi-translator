using System.Text.Json;
using Shizi.Popup.Host;
using Shizi.Popup.Services;

namespace Shizi.Popup.Bridge;

/// <summary>
/// Bridge 层：Rust→UI push 与 UI→Rust request。
/// push 解析与状态机由 <see cref="BridgeService"/> 处理。
/// </summary>
public sealed class NativeBridge
{
    public static NativeBridge Instance { get; } = new();

    private static readonly JsonSerializerOptions SerializeOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull,
    };

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

    /// <summary>是否已挂接 UI→Rust 写出（无 IPC 独立启动时为 false）。</summary>
    public bool HasRequestSink => _requestSink is not null;

    /// <summary>Rust → UI：接收 BridgePush / BridgeResponse JSON 字符串。</summary>
    public void ReceivePushJson(string json)
    {
        try
        {
            BridgeService.Instance.HandlePushJson(json);

            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            var typeName = root.TryGetProperty("type", out var t) ? t.GetString() ?? "?" : "?";
            _controller?.Window.OnBridgePushHint(typeName);
        }
        catch
        {
            _controller?.Window.OnBridgePushHint("push?");
        }
    }

    /// <summary>
    /// UI → Rust：发送 BridgeEnvelope JSON。
    /// 例：start_translation / ready / get_app_config。
    /// </summary>
    public void SendRequest(string envelopeJson)
    {
        _requestSink?.Invoke(envelopeJson);
    }

    /// <summary>便捷：构造标准 envelope 并发送（payload 使用 camelCase）。</summary>
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

        var json = JsonSerializer.Serialize(body, SerializeOptions);
        SendRequest(json);
    }

    /// <summary>UI 首帧可交互时调用（对齐 readyGate）。</summary>
    public void NotifyReady()
    {
        SendRequest("ready", requestId: Guid.NewGuid().ToString("N"));
    }
}
