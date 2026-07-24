using System.IO.Pipes;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.UI.Xaml;
using Shizi.Popup.Bridge;

namespace Shizi.Popup.Host;

/// <summary>
/// JSON 行协议宿主：接收 Rust 控制命令，回传 result / UI→Rust request。
/// 协议见 native/README.md。
/// </summary>
public sealed class IpcHost : IDisposable
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        PropertyNameCaseInsensitive = true,
    };

    private readonly PopupController _controller;
    private readonly NativeBridge _bridge;
    private readonly CancellationTokenSource _cts = new();
    private Stream? _stream;
    private StreamWriter? _writer;
    private readonly object _writeLock = new();
    private Task? _loop;

    private IpcHost(PopupController controller, NativeBridge bridge)
    {
        _controller = controller;
        _bridge = bridge;
        _bridge.SetRequestSink(SendRequestToRust);
    }

    public static IpcHost StartTcp(int port, PopupController controller, NativeBridge bridge)
    {
        var host = new IpcHost(controller, bridge);
        host._loop = Task.Run(() => host.RunTcpAsync(port));
        return host;
    }

    public static IpcHost StartNamedPipe(string pipeName, PopupController controller, NativeBridge bridge)
    {
        var host = new IpcHost(controller, bridge);
        host._loop = Task.Run(() => host.RunPipeAsync(pipeName));
        return host;
    }

    private async Task RunTcpAsync(int port)
    {
        try
        {
            using var client = new TcpClient();
            // Rust 先 listen 再 spawn；短暂重试连接
            Exception? last = null;
            for (var i = 0; i < 50; i++)
            {
                try
                {
                    await client.ConnectAsync(IPAddress.Loopback, port).ConfigureAwait(false);
                    last = null;
                    break;
                }
                catch (Exception ex)
                {
                    last = ex;
                    await Task.Delay(100).ConfigureAwait(false);
                }
            }

            if (last is not null || !client.Connected)
            {
                System.Diagnostics.Debug.WriteLine($"IPC TCP 连接失败: {last}");
                return;
            }

            _stream = client.GetStream();
            await RunReadLoopAsync().ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"IPC TCP 异常: {ex}");
        }
    }

    private async Task RunPipeAsync(string pipeName)
    {
        try
        {
            // 客户端连接 Rust 创建的命名管道
            var pipe = new NamedPipeClientStream(
                ".",
                pipeName,
                PipeDirection.InOut,
                PipeOptions.Asynchronous);

            Exception? last = null;
            for (var i = 0; i < 50; i++)
            {
                try
                {
                    await pipe.ConnectAsync(200).ConfigureAwait(false);
                    last = null;
                    break;
                }
                catch (Exception ex)
                {
                    last = ex;
                    await Task.Delay(100).ConfigureAwait(false);
                }
            }

            if (last is not null || !pipe.IsConnected)
            {
                System.Diagnostics.Debug.WriteLine($"IPC Pipe 连接失败: {last}");
                return;
            }

            _stream = pipe;
            await RunReadLoopAsync().ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"IPC Pipe 异常: {ex}");
        }
    }

    private async Task RunReadLoopAsync()
    {
        if (_stream is null)
        {
            return;
        }

        _writer = new StreamWriter(_stream, new UTF8Encoding(false), bufferSize: 64 * 1024)
        {
            AutoFlush = true,
            NewLine = "\n",
        };

        // 就绪握手
        WriteLine(new IpcMessage { Op = "hello" });

        using var reader = new StreamReader(_stream, Encoding.UTF8, detectEncodingFromByteOrderMarks: false, bufferSize: 64 * 1024, leaveOpen: true);
        while (!_cts.IsCancellationRequested)
        {
            string? line;
            try
            {
                line = await reader.ReadLineAsync().ConfigureAwait(false);
            }
            catch
            {
                break;
            }

            if (line is null)
            {
                break;
            }

            if (string.IsNullOrWhiteSpace(line))
            {
                continue;
            }

            HandleLine(line);
        }
    }

    private void HandleLine(string line)
    {
        IpcMessage? msg;
        try
        {
            msg = JsonSerializer.Deserialize<IpcMessage>(line, JsonOptions);
        }
        catch (Exception ex)
        {
            WriteLine(new IpcMessage { Op = "result", Ok = false, Error = $"JSON 解析失败: {ex.Message}" });
            return;
        }

        if (msg is null || string.IsNullOrEmpty(msg.Op))
        {
            WriteLine(new IpcMessage { Op = "result", Ok = false, Error = "缺少 op" });
            return;
        }

        try
        {
            switch (msg.Op)
            {
                case "ensure":
                    _controller.Ensure();
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "ensure" });
                    break;
                case "show":
                    _controller.Show(msg.X ?? 0, msg.Y ?? 0, msg.Mode ?? 1);
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "show" });
                    break;
                case "hide":
                    _controller.Hide();
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "hide" });
                    break;
                case "set_always_on_top":
                    _controller.SetAlwaysOnTop(msg.On ?? false);
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "set_always_on_top" });
                    break;
                case "set_size":
                    _controller.SetSize(msg.W ?? 420, msg.H ?? 200);
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "set_size" });
                    break;
                case "push_json":
                    if (msg.Data is not null)
                    {
                        _bridge.ReceivePushJson(msg.Data);
                    }
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "push_json" });
                    break;
                case "shutdown":
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "shutdown" });
                    _controller.Shutdown();
                    try
                    {
                        if (Application.Current is App app)
                        {
                            app.RequestShutdown();
                        }
                    }
                    catch
                    {
                        // best-effort
                    }
                    _cts.Cancel();
                    break;
                case "ping":
                    WriteLine(new IpcMessage { Op = "result", Ok = true, OpName = "ping" });
                    break;
                default:
                    WriteLine(new IpcMessage
                    {
                        Op = "result",
                        Ok = false,
                        OpName = msg.Op,
                        Error = $"未知 op: {msg.Op}",
                    });
                    break;
            }
        }
        catch (Exception ex)
        {
            WriteLine(new IpcMessage
            {
                Op = "result",
                Ok = false,
                OpName = msg.Op,
                Error = ex.Message,
            });
        }
    }

    private void SendRequestToRust(string envelopeJson)
    {
        WriteLine(new IpcMessage { Op = "request", Data = envelopeJson });
    }

    private void WriteLine(IpcMessage message)
    {
        lock (_writeLock)
        {
            if (_writer is null)
            {
                return;
            }

            try
            {
                var json = JsonSerializer.Serialize(message, JsonOptions);
                _writer.WriteLine(json);
            }
            catch
            {
                // ignore broken pipe
            }
        }
    }

    public void Dispose()
    {
        _cts.Cancel();
        try
        {
            _writer?.Dispose();
        }
        catch
        {
            // ignore
        }

        try
        {
            _stream?.Dispose();
        }
        catch
        {
            // ignore
        }
    }
}

/// <summary>IPC 行消息（控制面；Bridge 正文在 Data 字段，复用 envelope JSON）。</summary>
public sealed class IpcMessage
{
    public string Op { get; set; } = "";
    public bool? Ok { get; set; }
    public string? Error { get; set; }
    public string? OpName { get; set; }
    public double? X { get; set; }
    public double? Y { get; set; }
    public int? Mode { get; set; }
    public bool? On { get; set; }
    public double? W { get; set; }
    public double? H { get; set; }
    /// <summary>push_json / request 的 JSON 字符串载荷。</summary>
    public string? Data { get; set; }
}
