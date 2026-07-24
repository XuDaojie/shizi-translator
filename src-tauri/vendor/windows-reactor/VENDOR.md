# windows-reactor vendor 说明

## 来源

- 上游：`https://github.com/microsoft/windows-rs`
- rev：`884c9bbc1bd0a2315f00e0f04e34f6b1714653b9`
- 路径：`crates/libs/reactor`

## 本地补丁

`src/app.rs` → `init_app_platform`：

- **问题**：`SetProcessDpiAwarenessContext(PerMonitorV2)` 在进程 DPI 已被设置时返回 `ERROR_ACCESS_DENIED`（`0x80070005`）。
- **场景**：Tauri 使用的 **tao** 在创建事件循环时会先设置进程 DPI；随后路径 R 在专用 STA 上调用 `App::render` 会再次设置 → 硬失败，**`Application::Start` 根本不会执行**，WinUI 窗体不会创建。
- **修复**：对 `0x80070005` 视为「已由宿主设置 DPI」，继续 `CoInitializeEx` / `Application::Start`。

## 上游跟踪

- Issue：https://github.com/microsoft/windows-rs/issues/4742  
  （`App::run` 在进程 DPI 已设置时 0x80070005，嵌入 Tauri/tao 失败）

## 回退上游

上游若合并等价修复（见 #4742），可将 `src-tauri/Cargo.toml` 的 `windows-reactor` 改回 git rev 依赖，并删除本目录。

## 依赖

`Cargo.toml` 将 workspace 依赖改为同一 rev 的 git `package = "..."` 解析，与 `windows-reactor-setup` 共用上游 monorepo。
