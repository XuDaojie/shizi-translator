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

# 单测
dotnet test native/windows/popup/Shizi.Popup.Tests/Shizi.Popup.Tests.csproj -c Release
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

## 打包布局（任务 12）

安装包 / release 运行时，Rust `host.rs` 查找顺序：

1. **可执行文件旁** `popup-native/Shizi.Popup.exe`（NSIS 安装布局；优先）
2. 可执行文件旁直接 `Shizi.Popup.exe`
3. 相对 `CARGO_MANIFEST_DIR` 的 dev 输出（`bin/x64/{Release,Debug}/net8.0-.../win-x64/`）

### 推荐流程

```bash
# 开发：tauri dev 的 beforeDevCommand 会自动跑 native:dev
# （Debug 构建 + stage；源码未变可跳过 rebuild）
npm run tauri dev
# 改了 native/ 且 dev 已在跑：再执行一次，脚本会结束残留 Shizi.Popup
npm run native:dev

# 发版 / 安装包
npm run native:release
npm run tauri build   # beforeBuildCommand 也会再跑 native:release
```

| 项 | 说明 |
|----|------|
| npm 入口 | **`native:dev`** / **`native:release`**（不绑「弹窗」语义；后续其它原生窗也可由同一脚本扩展） |
| 脚本 | `scripts/build-native.ps1`：`-Configuration Debug\|Release`；stage 到 `resources/popup-native` 与 `target/{debug,release}/popup-native` |
| 磁盘目录 `popup-native/` | 当前为**翻译弹窗宿主**的旁路目录名（`host.rs` 查找约定）；不是 npm 命令名。多原生面时再演进布局 |
| `tauri.conf.json` | **beforeDevCommand** = `native:dev && npm run dev`；**beforeBuildCommand** 含 `native:release` |
| `build.rs` | Windows 上 best-effort `dotnet build`；Release 时同步到上述 stage 路径 |
| 体积 | **WASDK 自包含**，整树可达数百 MB；目录已 gitignore，勿提交 |
| 严格模式 | `SHIZI_POPUP_NATIVE_STRICT=1` 时脚本失败非 0 退出（发版流水线可用） |
| 强制重编 | `SHIZI_BUILD_WINUI=1` 让 `build.rs` 强制 `dotnet build` |

未装 .NET / 构建失败时：脚本仍创建占位目录以免 `tauri build` 因 resources 路径缺失中断；此时 WinUI 不可用，默认 `popupUi: webview` 不受影响。

## CI

`.github/workflows/ci.yml` 的 **backend** job（`windows-latest`）：

1. `actions/setup-dotnet@v5` → `8.0.x`
2. `dotnet build` `Shizi.Popup.csproj` Release x64
3. `dotnet test` `Shizi.Popup.Tests`
4. 既有 `cargo test` / `cargo build`

## 手工验收

### 开发联调

```bash
dotnet build native/windows/popup/Shizi.Popup.csproj -c Release -p:Platform=x64
# 无 IPC：直接看窗口壳
./native/windows/popup/bin/x64/Release/net8.0-windows10.0.19041.0/win-x64/Shizi.Popup.exe

# 与主程序联调：设置 popupUi=winui 后 npm run tauri dev
# ensure 失败会会话回退 webview（不写回 config）
```

### 验收清单（任务 12.D）

| # | 项 | 状态 |
|---|----|------|
| 1 | 默认 `popupUi: webview`，行为与现网一致 | 待手工 |
| 2 | 设置改 winui → 保存 → **下次**划词出原生窗 | 待手工 |
| 3 | 多服务流式 / 取消 / 重试 / 语言切换 / 图钉 / 设置 / 截图译 / 关窗 hide | 待手工 |
| 4 | 两后端不同时可见 | 待手工 |
| 5 | 模拟 ensure 失败 → 回退 webview，config 仍为 winui | 待手工 |
| 6 | 安装包内存在 `popup-native/Shizi.Popup.exe` 且 winui 可启动 | 待手工（tauri 联调） |
| 7 | CI：dotnet build/test + cargo test 绿 | 自动化 |

## 目录

```
native/windows/popup/
  Shizi.Popup.csproj
  Program.cs / App.xaml* / MainWindow.xaml*
  Host/PopupController.cs   # 窗壳操作
  Host/IpcHost.cs           # TCP / named pipe
  Host/PopupExports.cs      # C ABI 语义名对照
  Bridge/NativeBridge.cs    # Bridge 接线
  State/                    # 翻译状态机
  Services/                 # 朗读等 UI 侧服务
  Shizi.Popup.Tests/        # xUnit
```
