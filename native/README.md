# native — 原生 UI 宿主

当前仅包含 Windows 翻译弹窗（WinUI 3）。

## Transport（任务 8 决策）

| 项 | 值 |
|----|-----|
| **Transport** | **subprocess + localhost TCP**（JSON 行协议） |
| 备选 | 命名管道 `\\.\pipe\shizi-popup-{pid}`（C# 已实现 `--pipe`） |
| 未采用 | 进程内 hostfxr 加载 WinUI（非 main EXE + WASDK 启动不可靠） |

配置项 `popupUi: winui` 与 Rust `PopupUi` trait **不变**；`WinUiPopupUi` 内部走子进程 IPC。

语义对齐的 C ABI 操作名（经 IPC `op` 映射，非真正 DLL 导出）：

| op / 语义 | 说明 |
|-----------|------|
| `ensure` | 建/保活隐藏窗 |
| `show` | `x,y,mode`；mode 0 NearCursor / 1 Restore |
| `hide` | hide 不销毁 |
| `set_always_on_top` | 图钉 |
| `set_size` | 逻辑像素 `w,h` |
| `push_json` | Rust→UI，`data` 为 BridgePush JSON 字符串 |
| `shutdown` | best-effort |
| `hello` / `result` / `request` | 握手、应答、UI→Rust BridgeEnvelope |

Bridge 请求/推送信封与 `popup_bridge` 协议一致（`bridgeVersion: 1`）。

## 构建

```bash
# 推荐：Release（.NET + WASDK 自包含）
dotnet build native/windows/popup/Shizi.Popup.csproj -c Release -p:Platform=x64

# Debug（WASDK 仍自包含；.NET 可为 framework-dependent）
dotnet build native/windows/popup/Shizi.Popup.csproj -c Debug -p:Platform=x64
```

### 自包含要点 / Runtime 弹窗

- `WindowsAppSDKSelfContained=true`（Debug + Release）
- `WindowsAppSdkBootstrapInitialize=false`（**不要**走系统 Bootstrap）
- NuGet：`Microsoft.WindowsAppSDK` **1.5.240802000**

**为何会弹「requires Windows App SDK runtime version 1.5」？**

| 包 | 作用 |
|----|------|
| `Microsoft.WindowsAppRuntime.1.5` | 框架本体（很多人「有」的是这个） |
| `MicrosoftCorporationII.WinAppRuntime.Main.1.5` | unpackaged + **系统 Bootstrap** 时必需 |

本机常见：有 Runtime.1.5、**没有** Main.1.5，且若 `WindowsAppSdkBootstrapInitialize=true` 或 Debug 未自包含 WASDK，就会弹安装框——**不是「完全没装 Runtime」**。

当前工程用 **WASDK 自包含 + 关闭 Bootstrap 自动初始化**，不依赖 Main 包。联调请优先用 **Release 产物**（Rust host 也优先找 Release）。

### 产物路径

Platform=x64 时：

```
native/windows/popup/bin/x64/Release/net8.0-windows10.0.19041.0/win-x64/Shizi.Popup.exe
native/windows/popup/bin/x64/Debug/net8.0-windows10.0.19041.0/win-x64/Shizi.Popup.exe
```

Rust 查找顺序：

1. 可执行文件旁 `popup-native/Shizi.Popup.exe`（安装/打包布局，任务 12）
2. 相对 `CARGO_MANIFEST_DIR` 的上述 Debug/Release 输出（含 `bin/x64/...`）

`src-tauri/build.rs` 在 Windows 上可选调用 `dotnet build`（已有产物则跳过；`SHIZI_BUILD_WINUI=1` 强制；失败仅 warn）。

## 手工验收

```bash
# 无 IPC：直接看窗口壳
./native/windows/popup/bin/x64/Release/net8.0-windows10.0.19041.0/win-x64/Shizi.Popup.exe

# 与主程序联调：设置 popupUi=winui 后 npm run tauri dev
# ensure 失败会会话回退 webview（不写回 config）
```

## 目录

```
native/windows/popup/
  Shizi.Popup.csproj
  Program.cs / App.xaml* / MainWindow.xaml*
  Host/PopupController.cs   # 窗壳操作
  Host/IpcHost.cs           # TCP / named pipe
  Host/PopupExports.cs      # C ABI 语义名对照
  Bridge/NativeBridge.cs    # Bridge 最小接线
```

任务 9：状态机与单测；任务 10：完整翻译 UI 区块。
