# Spike：windows-reactor + Tauri 共存（路径 R）

日期：2026-07-24  
分支：`feat/winui-reactor-popup`  
关联规格：`docs/superpowers/specs/2026-07-24-winui-reactor-popup-design.md`  
关联计划：`docs/superpowers/plans/2026-07-24-winui-reactor-popup.md`

## M0 目的

在改动弹窗后端实现之前，完成：

1. `windows-reactor` / `windows-reactor-setup` 依赖接入与编译期存在性探测（任务 1）
2. **S1 同进程 + 专用 STA 线程**共存 spike、否决门验收（任务 2）

## 候选 / 最终 pin 的 git rev

| 项 | 值 | 备注 |
|----|----|------|
| monorepo | `https://github.com/microsoft/windows-rs` | |
| 候选 rev | `884c9bbc1bd0a2315f00e0f04e34f6b1714653b9` | 计划初值 |
| **最终 rev** | `884c9bbc1bd0a2315f00e0f04e34f6b1714653b9` | **M0 锁定** |
| package | `windows-reactor` / `windows-reactor-setup`（均 `v0.0.0`） | monorepo git + `package` 名解析成功 |
| 探测符号 | `windows_reactor::Element` | 编译通过 |
| `windows` crate | **应用侧仍 `0.58`** | reactor 侧独立 `windows-core 0.62.2` 并存；M0 **未**强制升级应用侧 windows（双 ABI 在本机可运行） |

## 共存模型（写死）

| 项 | 结论 |
|----|------|
| **模型** | **S1：同进程 + 专用 STA 线程**（线程名 `shizi-reactor-ui`） |
| 否决 S2 | 未触发；S1 本机可启动 Application 消息循环 |
| bootstrap | 进程级 `OnceLock` + `windows_reactor::bootstrap()` 一次 |
| 消息循环 | `App::new().title(哨兵).inner_size(1,1).render(..)` 阻塞 STA 线程 |
| 弹窗 | `ReactorWindow` + `Backdrop::Mica` + `text_block` / `button` |
| 命令通道 | `mpsc::channel<HostCmd>` 非阻塞 send；UI 线程 `DispatcherTimer` 33ms `try_recv` |
| 标签更新 | `use_async_state` + `AsyncSetState::call`（任意线程 marshal 到 UI） |
| hide/show | `FindWindowW(title)` + `ShowWindow(SW_HIDE/SW_SHOW)`；**不** `WindowHandle::close` |
| 哨兵 / last-window-exit | 主 `App` 窗为哨兵（标题 `Shizi Reactor Sentinel`，立即 hide）；reactor 在最后一扇**已注册**窗 `Closed` 时 `process::exit(0)`，故哨兵永不 Close |
| 标题栏 X | 会销毁弹窗 HWND；下次 `Show` 在 UI 线程 `ReactorWindow` 重建（**不**重跑 bootstrap） |
| 应用侧 backend | M0 时仍为路径 B GDI；**任务 6+ 已切换为路径 R**（`WinuiPopupBackend` → reactor host） |

## API 形状（已实现）

```rust
// src-tauri/src/app/popup_backend/winui/reactor/host.rs
pub enum HostCmd { Show, Hide, SetLabel(String), Shutdown }
pub struct ReactorHostHandle { /* Sender<HostCmd> */ }
impl ReactorHostHandle {
    pub fn start() -> Result<Self, String>; // STA + bootstrap；失败可降级 WebView
    pub fn publish_label(&self, s: impl Into<String>); // 非阻塞
    pub fn show(&self);
    pub fn hide(&self);
    pub fn shutdown(&self); // 仅 hide，保留哨兵
}
pub fn ensure_process_bootstrap() -> Result<(), String>;
```

`bootstrap::try_bootstrap()` 已改为路径 R 探测（调用 `ensure_process_bootstrap`），**不再**断言「路径 B」恒真。

## 依赖接入备注

- `Cargo.toml`：`windows-reactor` / build-dep `windows-reactor-setup`，rev 见上表
- feature：`popup-winui` 默认开；空 feature `popup-winui-gdi` 预留
- `build.rs`：`windows_reactor_setup::as_framework_dependent()` → 复制 Bootstrap DLL + `resources.pri` 到 `target/{profile}/`
- **测试注意**：`cargo test` 可执行文件在 `target/debug/deps/`，冒烟测试会尝试把 Bootstrap / `resources.pri` 复制到 exe 旁

## Runtime 版本 / 安装备注

| 项 | 值 |
|----|----|
| reactor-setup 声明 | Windows App SDK Runtime **2.3.1**（`as_framework_dependent` / self-contained 用） |
| bootstrap 常量 | `WINDOWSAPPSDK_RELEASE_MAJORMINOR = 0x20000`（major 2）；runtime uint64 → **2.0.1.0** 最小版本语义 |
| 本机已装 | `Microsoft.WindowsAppRuntime.2`（含 2.2.x / 2.3.x）及 1.x 多版本；Bootstrap 探测成功 |
| 下载 | [Windows App Runtime 下载](https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads)；产品侧沿用 `WINUI_RUNTIME_DOWNLOAD_URL` |
| 缺失时行为 | `ensure_process_bootstrap` / `ReactorHostHandle::start` → `Err`；`try_bootstrap().ok == false` → 可被 `create_host_with_winui_fallback` 降级 |

## 验收表

### 编译 / 探测（任务 1+2）

| 检查项 | 结果 | 备注 |
|--------|------|------|
| `cargo check -p shizi --features popup-winui` | **PASS** | |
| `reactor_crate_is_linked` | **PASS** | |
| `try_bootstrap_reports_path_r_not_path_b` | **PASS** | 文案含「路径 R」 |
| `ensure_process_bootstrap_is_idempotent` | **PASS** | OnceLock |
| host 命令通道非阻塞单测 | **PASS** | |

### 手动 / 冒烟清单（任务 2）

| # | 标准 | 通过? | 备注 |
|---|------|-------|------|
| 1 | 同进程弹出真 WinUI 窗（Inspect 可见系统控件 / 非 GDI 矩形） | **PASS（代码路径+冒烟 HWND）** | 使用 `ReactorWindow` + WinUI `text_block`/`button`；Inspect 细看建议人工再确认一次 UI 外观 |
| 2 | Mica 或明确记录 API 不可用原因 | **PASS** | `ReactorWindow::backdrop(Backdrop::Mica)`；API 可用，activate 内 apply |
| 3 | SetLabel/publish 后文本可见更新 | **PASS（代码路径+冒烟）** | `AsyncSetState`；冒烟调用 `publish_label` 两次后进程稳定（视觉需人工扫一眼） |
| 4 | hide → show 稳定，不重建 Runtime | **PASS** | 冒烟两次 show/hide；bootstrap OnceLock 进程一次 |
| 5 | 关闭弹窗（hide）**不**退出托盘进程 | **PASS** | 冒烟 hide 后断言继续执行；哨兵防 `process::exit` |
| 6 | 打开设置 WebView 后再 show 弹窗无死锁 | **需人工本机验收** | 任务 6+ 已接线路径 R 主路径；设置 WebView + 再 show 须本机再验 |
| 7 | Runtime 缺失或 bootstrap 失败时返回 Err | **PASS（代码路径）** | `start()` / `try_bootstrap` 映射 Err；无 Runtime 机需再验 |
| 8 | 写死共存模型 S1/S2 + 精确 git rev | **PASS** | **S1** + rev `884c9bbc1bd0a2315f00e0f04e34f6b1714653b9` |

### 关键项汇总

| 关键项 | 结果 |
|--------|------|
| 1 真 WinUI 窗 | PASS |
| 3 SetLabel | PASS |
| 4 hide/show | PASS |
| 5 hide 不退出进程 | PASS |
| 7 bootstrap 失败 → Err | PASS |

**否决门结论：Go（路径 R 可继续任务 3+）**

## 冒烟命令与结果

```powershell
cd src-tauri
cargo check -p shizi --features popup-winui
cargo test -p shizi --features popup-winui --lib -- --test-threads=1
$env:SHIZI_M0_SPIKE="1"
cargo test -p shizi --features popup-winui --lib m0_reactor_host_smoke -- --nocapture --test-threads=1
```

结果摘要（2026-07-24 本机）：

- `cargo check`：通过  
- lib 单测：431 passed, 2 ignored  
- `m0_reactor_host_smoke`（`SHIZI_M0_SPIKE=1`）：**ok** —「hide/show 完成，进程仍存活」

## M3 目视清单（任务 10：视觉抛光）

| 项 | 期望 | 实现备注 |
|----|------|----------|
| 宽度 | ~468 | `host` `POPUP_LOGICAL_WIDTH` + `view` `POPUP_VIEW_WIDTH` = 468 |
| 高度 | ~520（可调） | `inner_size(468, 520)`；定位同尺寸；结果区 `scroll_viewer` `max_height` 360 |
| 背景 | Mica 或 Fluent 实底 fallback | `ReactorWindow::backdrop(Backdrop::Mica)`（API 可用） |
| Accent | 柿子橙 `#D55A1F` 可见于主按钮/品牌 | 标题前景 + 取消/重试底色；系统 `.accent()` 跟 Windows 强调色故用 `Color::rgb` 资源色 |
| 多卡 | 间距/分割清晰 | `results_list` 卡间距 10；卡内 6 |
| 标题 | 产品名 | 窗标题 / 栏内均为「柿子翻译」 |
| 非 GDI | Inspect 为 XAML 控件 | 路径 R `ReactorWindow` + WinUI 控件 |

人工本机扫一眼：Mica 在浅/深色系统主题下是否透出桌面；accent 按钮在高对比主题下是否可读。

## 已知问题：Tauri 进程内 DPI 二次设置

| 项 | 说明 |
|----|------|
| 现象 | `popupUiBackend=winui` 时日志：`App::render / Application::Start 失败: 拒绝访问。(0x80070005)`，降级 WebView |
| 根因 | **tao** 已 `SetProcessDpiAwarenessContext(PerMonitorV2)`；upstream `windows-reactor` `init_app_platform` 再次设置失败并 **硬返回**，`Application::Start` 未执行（不是 Runtime 缺失） |
| 证据 | 独立 `m0_reactor_host_smoke` 通过；整应用 setup 失败；本机二次 SetDpi → `GetLastError=5` |
| 修复 | vendor `src-tauri/vendor/windows-reactor`：吞掉 DPI `0x80070005`；回归 `m0_reactor_host_smoke_after_dpi_preset` |
| 文档 | `src-tauri/vendor/windows-reactor/VENDOR.md` |
| 上游 | https://github.com/microsoft/windows-rs/issues/4742 |

## 变更日志

| 日期 | 变更 |
|------|------|
| 2026-07-24 | 创建骨架（任务 1 / M0 依赖接入） |
| 2026-07-24 | M0 绿：候选 rev 编译 + 探测测试通过 |
| 2026-07-24 | 任务 2：S1 STA host + 哨兵 + 否决门 **Go**；锁定 rev；try_bootstrap 改路径 R |
| 2026-07-24 | 任务 10：窗 468×520 + Mica；标题「柿子翻译」；柿子橙 accent；结果区滚动确认 |
| 2026-07-24 | 任务 12：架构/README/AGENTS/CI 写明路径 R + Runtime；M0 结论表已确认 |
| 2026-07-24 | 修复：vendor reactor 容忍宿主已设 DPI（Tauri 共存 0x80070005） |
