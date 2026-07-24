namespace Shizi.Popup.Host;

/// <summary>
/// C ABI 语义名对照表（任务 8）。
/// 当前 transport = 子进程 IPC，不真正导出 UnmanagedCallersOnly；
/// 方法名与 Rust/IPC <c>op</c> 对齐，便于任务 9+ 或未来 in-proc 复用。
/// </summary>
public static class PopupExports
{
    // shizi_popup_initialize → 子进程启动 + hello / 注册 request sink（IpcHost）
    // shizi_popup_ensure → PopupController.Ensure / op=ensure
    // shizi_popup_show → PopupController.Show / op=show
    // shizi_popup_hide → PopupController.Hide / op=hide
    // shizi_popup_set_always_on_top → PopupController.SetAlwaysOnTop
    // shizi_popup_set_size → PopupController.SetSize
    // shizi_popup_push_json → NativeBridge.ReceivePushJson / op=push_json
    // shizi_popup_shutdown → PopupController.Shutdown / op=shutdown
    // shizi_popup_is_available → 进程可启动即 1（Rust 侧查 exe 存在）

    public const string AbiEnsure = "shizi_popup_ensure";
    public const string AbiShow = "shizi_popup_show";
    public const string AbiHide = "shizi_popup_hide";
    public const string AbiSetAlwaysOnTop = "shizi_popup_set_always_on_top";
    public const string AbiSetSize = "shizi_popup_set_size";
    public const string AbiPushJson = "shizi_popup_push_json";
    public const string AbiShutdown = "shizi_popup_shutdown";
    public const string AbiIsAvailable = "shizi_popup_is_available";
    public const string AbiInitialize = "shizi_popup_initialize";
}
