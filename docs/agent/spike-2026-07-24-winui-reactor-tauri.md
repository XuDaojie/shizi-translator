# Spike：windows-reactor + Tauri 共存（路径 R）

日期：2026-07-24  
分支：`feat/winui-reactor-popup`  
关联规格：`docs/superpowers/specs/2026-07-24-winui-reactor-popup-design.md`  
关联计划：`docs/superpowers/plans/2026-07-24-winui-reactor-popup.md`

## M0 目的

在改动弹窗后端实现之前，完成 **windows-reactor 依赖接入** 与 **编译期存在性探测**，作为否决门前半：

1. `windows-reactor` / `windows-reactor-setup` 能从 monorepo git 解析并参与编译。
2. `cargo check -p shizi --features popup-winui` 无 error。
3. 探测测试 `reactor_crate_is_linked` 通过。
4. 不在本阶段实现 host STA 线程、不改 `WinuiPopupBackend`、不删 GDI `ui.rs`。

任务 2 再做 STA 共存 spike 与完整否决门结论。

## 候选 / 最终 pin 的 git rev

| 项 | 值 | 备注 |
|----|----|------|
| monorepo | `https://github.com/microsoft/windows-rs` | |
| 候选 rev | `884c9bbc1bd0a2315f00e0f04e34f6b1714653b9` | 计划初值 |
| **最终 rev** | `884c9bbc1bd0a2315f00e0f04e34f6b1714653b9` | 候选一次拉通；任务 2 可再正式锁定 |
| package | `windows-reactor` / `windows-reactor-setup`（均 `v0.0.0`） | monorepo git + `package` 名解析成功 |
| 探测符号 | `windows_reactor::Element` | 编译通过，无需改名 |
| `windows` crate | `0.58`（现有） | reactor 引入独立 `windows-core 0.62.2` 等；未强制升级应用侧 `windows` |

## 依赖接入备注

- `Cargo.toml`：`[target.'cfg(windows)'.dependencies]` 增加 `windows-reactor`；`[target.'cfg(windows)'.build-dependencies]` 增加 `windows-reactor-setup`。
- feature：`popup-winui` 保持默认开启；新增空 feature `popup-winui-gdi`（为后续 GDI 回退预留，本任务不接线）。
- `build.rs`：在 `tauri_build::build()` 之后，Windows + `popup-winui` 下调用 `windows_reactor_setup::as_framework_dependent()`（framework-dependent，与 v1 发布模型一致）。
- `windows` crate：以 `cargo check` 报错为准合并 reactor 所需 features，**保留**现有 OCR/截图 features。
- 仅接线：`winui/mod.rs` 增加 `mod reactor;`，不改 backend / GDI。

## 验收表（任务 2 填写）

| 检查项 | 结果 | 备注 |
|--------|------|------|
| `cargo check -p shizi --features popup-winui` | PASS（M0） | 默认 feature 含 `popup-winui` |
| `reactor_crate_is_linked` | PASS（M0） | `windows_reactor::Element` |
| STA 宿主可创建（无 WinUI 控件） | （任务 2） | |
| 与 Tauri WebView 同进程共存 | （任务 2） | |
| 否决门结论（Go / No-Go） | （任务 2） | |

## 依赖解析摘录（M0）

`cargo` 从候选 rev 锁定（节选）：

- `windows-reactor v0.0.0` / `windows-reactor-setup v0.0.0`
- 伴随：`windows-core v0.62.2`、`windows-composition`、`windows-collections`、`windows-future` 等（均来自同一 git rev）
- 应用侧 `windows = 0.58` **未改**；OCR/截图 features 列表原样保留

`build.rs`：Windows + `popup-winui` 下调用 `windows_reactor_setup::as_framework_dependent()`。

## 变更日志

| 日期 | 变更 |
|------|------|
| 2026-07-24 | 创建骨架（任务 1 / M0 依赖接入） |
| 2026-07-24 | M0 绿：候选 rev 编译 + 探测测试通过；文档填写 pin 与验收前两行 |
