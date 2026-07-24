# 翻译弹窗路径 R（windows-reactor 真 WinUI 3）实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将配置 `popupUiBackend=winui` 的实现从路径 B（Win32 + GDI）升级为路径 R（`windows-reactor` 真 WinUI 3 控件），保持 `PopupBackend`/`PopupHost` 契约与 WebView 降级，视觉对齐 Open Design `#popup-winui3`。

**架构：** 在现有 `WinuiPopupBackend` 内替换窗口宿主：专用 STA 线程（首选 S1）bootstrap Windows App SDK + Reactor 控件树；`publish` 经线程安全快照投递到 UI 线程 hooks 驱动重绘；控件事件映射为既有 `PopupUserAction` → `actions::handle_user_action`。`popup-winui` feature 语义升级为路径 R；GDI 代码迁移期 feature 门控，M4 删除或移入 legacy。

**技术栈：** Tauri 2 / Rust、`windows-reactor` + `windows-reactor-setup`（git 依赖锁定 rev，未上 crates.io）、`windows` crate（与 reactor 对齐版本）、Windows App Runtime（framework-dependent）、cargo test

**规格来源：** `docs/superpowers/specs/2026-07-24-winui-reactor-popup-design.md`

---

## 与 spec 的实现澄清（写死未决项）

### 1. 与 Tauri 共存模型

| 项 | 决定 |
|----|------|
| 首选 | **S1：同进程 + 专用 STA 线程**跑 Reactor 消息循环 |
| 备选 | **S2：主线程集成**——仅当 M0 证明 S1 不可行且 S2 无死锁 |
| 失败 | **S3**：保留 WebView 为唯一可用弹窗；**不宣称** winui=真 WinUI；GDI 可临时保留但不得冒充路径 R |
| 硬约束 | 禁止在全局快捷键同步栈重初始化；`publish` 非阻塞；`hide`/`show` 幂等；进程级 bootstrap 一次 |

**S1 目标形状（M0 验证后可微调，但接口保持）：**

```
Tauri 主线程                          Reactor STA 线程
─────────────                         ────────────────
PopupHost.lock                        bootstrap() 一次
  backend.publish(vm)  ──queue──►     更新 hooks state → re-render
  backend.show/hide    ──queue──►     ReactorWindow show/hide
  backend.ensure       ──oneshot─►    起线程 / 建窗 / 回传 ok|err
UI 事件                ◄──action──     on_click → handle_user_action
```

**Reactor 特有陷阱（M0 必须验证）：**

1. 官方 `App::new().render(..)` 会占用消息循环，且文档写明 **最后一扇窗关闭会退出进程**——托盘应用 **禁止** 因关弹窗退出。M0 须找到可工作的形态，例如：
   - STA 线程上跑 `App` 时保留 **永不销毁的哨兵根**（隐藏 1×1 或最小化 host），弹窗用 `ReactorWindow` 二次开窗；或
   - 仅用可控制生命周期的 API，使 `hide` 不触发 last-window-exit。
2. 所有 WinUI/Reactor 调用仅限 STA UI 线程。
3. 与 WebView2 设置窗同进程并存：打开设置 → 再 show 弹窗 → hide，无死锁。

### 2. 依赖版本（精确 pin 由 M0 产出）

| 项 | 计划起点 | 写死方式 |
|----|----------|----------|
| `windows-reactor` | git `https://github.com/microsoft/windows-rs`，path `crates/libs/reactor` | M0 成功编译后 **锁定 commit SHA** 写入 `Cargo.toml` 与本计划「M0 结论」表 |
| `windows-reactor-setup` | 同上 monorepo path `crates/libs/reactor-setup` | 同 rev |
| `windows` | 当前 `0.58` **可能不足**；以 reactor 工作区要求为准 **统一升级** | 禁止双版本 ABI；OCR/截图等既有 `windows` features 在升级后补齐编译 |
| crates.io | reactor **未发布**时只用 git | 日后正式版可跟进，另开任务 |

候选 rev（执行 M0 时先试，失败则 `git log` 换已知绿 commit）：

```
# 计划编写时 master HEAD 参考（非最终 pin）
# 884c9bbc1bd0a2315f00e0f04e34f6b1714653b9
```

### 3. 源文可编辑策略（v1）

| 项 | 决定 |
|----|------|
| 源文 | **只读展示** + 可选系统文本选择/复制（`text_block` selectable 或只读 `text_box`） |
| 重译触发 | **不**就地编辑重译；换语言走 `SetSessionLanguages`（与现网） |
| 源文内容 | 以会话 `PopupViewModel.source_text` 为准 |

### 4. 发布 / Runtime 模型（v1）

| 项 | 决定 |
|----|------|
| 部署 | **framework-dependent**：`build.rs` 在 `popup-winui` 时调用 `windows_reactor_setup::as_framework_dependent()`（或 M0 确认的等价 API） |
| 本机开发 | 安装对应 Windows App Runtime；README 写明版本/下载页 |
| NSIS | v1 **不改** installer 捆绑完整 Runtime |
| self-contained | **非 v1 默认**；可作为后续可选 feature/文档段落 |
| 降级 URL | 沿用 `WINUI_RUNTIME_DOWNLOAD_URL` |

### 5. feature 矩阵

```toml
# src-tauri/Cargo.toml
[features]
default = ["popup-winui"]
popup-winui = []                 # 路径 R：windows-reactor
popup-winui-gdi = []             # 可选：迁移期编译 GDI 对照；默认不启
```

| 场景 | 行为 |
|------|------|
| Windows + `popup-winui` | `WinuiPopupBackend` = 路径 R |
| Windows + `popup-winui` + `popup-winui-gdi` | 仅开发对照；**产品配置仍只认 winui=R**，GDI 不走配置枚举 |
| `--no-default-features` | 无原生后端；`winui` 配置运行时回退 webview |
| 非 Windows | 不编译 reactor |

### 6. 本轮明确不做

- 设置 / OCR / overlay 迁 Reactor  
- macOS / Linux 原生弹窗  
- backend 热切换  
- .NET / C# / XAML 文件  
- 托盘菜单 Reactor 化  
- NSIS 内嵌完整 Runtime  
- 与 WebView Bob 风像素级一致  
- 源文就地编辑触发重译  

---

## 文件结构

| 文件 | 职责 |
|------|------|
| 修改 `src-tauri/Cargo.toml` | git 依赖 `windows-reactor` / setup；`windows` 版本对齐；features |
| 修改 `src-tauri/build.rs` | `tauri_build` + `cfg(popup-winui)` 时 `as_framework_dependent()` |
| 修改 `src-tauri/src/app/popup_backend/mod.rs` | 注释路径 R；导出不变 |
| 修改 `src-tauri/src/app/popup_backend/winui/mod.rs` | 子模块：`reactor/`；GDI `ui` 门控 |
| 修改 `src-tauri/src/app/popup_backend/winui/backend.rs` | trait 实现改调 reactor host |
| 修改 `src-tauri/src/app/popup_backend/winui/bootstrap.rs` | 真实 bootstrap / 失败信息（替换「路径 B 恒 Ok」） |
| 修改 `src-tauri/src/app/popup_backend/winui/actions.rs` | 复制文案从 `reactor/state` 取快照；去掉对 GDI `ui::load_paint_snapshot` 的硬依赖 |
| 创建 `src-tauri/src/app/popup_backend/winui/reactor/mod.rs` | reactor 子模块出口 |
| 创建 `src-tauri/src/app/popup_backend/winui/reactor/host.rs` | STA 线程、命令队列、ensure/show/hide/destroy/publish |
| 创建 `src-tauri/src/app/popup_backend/winui/reactor/state.rs` | `SharedPopupState`、快照、`resolve_copy_text` 等纯函数 |
| 创建 `src-tauri/src/app/popup_backend/winui/reactor/view.rs` | `fn render(cx) -> Element` 五区 UI |
| 创建 `src-tauri/src/app/popup_backend/winui/reactor/meta.rs` | 卡片 model/tokens 规则（对齐 `resultCardMeta` / 现 GDI `card_detail_label`） |
| 创建 `src-tauri/src/app/popup_backend/winui/reactor/langs.rs` | `LANG_TABLE`、display name、swap 规则（从 `ui.rs` 抽出） |
| 迁移期保留 `src-tauri/src/app/popup_backend/winui/ui.rs` | `#[cfg(feature = "popup-winui-gdi")]`；M4 删除或 `legacy/` |
| 修改 `.github/workflows/ci.yml` | Windows job 装 Runtime 或文档化依赖；`cargo test` default features |
| 修改 `docs/agent/architecture-notes.md` | winui = 路径 R |
| 修改 `README.md` / `AGENTS.md` / `CLAUDE.md` | Runtime 开发依赖；弹窗后端说明 |
| 创建 `docs/agent/spike-2026-07-24-winui-reactor-tauri.md` | M0 结论：S1/S2、rev pin、否决记录 |

**刻意不改：** `pot-desktop/`、翻译 core 协议、历史 schema、设置页字段名（仍 `webview`\|`winui`）、OCR/overlay WebView。

---

## 里程碑映射

| 里程碑 | 任务 | 出口 |
|--------|------|------|
| **M0 Spike（否决门）** | 任务 1–2 | 可演示计数器级窗 + 共存 + 降级；**未通过则停止全量 UI** |
| **M1 契约** | 任务 3–6 | `WinuiPopupBackend` 走 Reactor；源文 + 单卡 + 关闭/复制 |
| **M2 五区** | 任务 7–9 | 语言栏 + 多卡 + 状态栏 + 必接动作 |
| **M3 抛光** | 任务 10 | Mica/accent/tokens/滚动；目视接近原型 |
| **M4 清理** | 任务 11–12 | 文档/CI；移除或隔离 GDI |

---

## 任务 1：M0 依赖接入 + 编译 spike 模块

**文件：**
- 修改：`src-tauri/Cargo.toml`
- 修改：`src-tauri/build.rs`
- 创建：`src-tauri/src/app/popup_backend/winui/reactor/mod.rs`（最小）
- 创建：`docs/agent/spike-2026-07-24-winui-reactor-tauri.md`（骨架）

- [ ] **步骤 1：编写失败的依赖探测测试（文档化预期）**

在 `bootstrap` 测试旁先 **不**改实现；本任务以「能 `cargo check -p shizi --features popup-winui`」为门。若当前无 reactor 符号，先加：

```rust
// src-tauri/src/app/popup_backend/winui/reactor/mod.rs
//! 路径 R：windows-reactor 宿主（M0+）

#![cfg(all(windows, feature = "popup-winui"))]

/// M0：是否已链接 windows-reactor（编译期存在性）。
#[cfg(test)]
mod tests {
    #[test]
    fn reactor_crate_is_linked() {
        // 使用任意稳定 re-export；若 API 更名，M0 按编译器错误改这一行即可
        let _ = std::any::type_name::<windows_reactor::Element>();
        assert!(!std::any::type_name::<windows_reactor::Element>().is_empty());
    }
}
```

- [ ] **步骤 2：运行测试验证失败（未加依赖时）**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui reactor_crate_is_linked -- --nocapture
```

预期：FAIL（找不到 `windows_reactor`）或模块未编入。

- [ ] **步骤 3：写入 git 依赖与 build.rs（最少让编译过）**

`Cargo.toml` 方向（rev 先用候选，M0 绿后锁定）：

```toml
[features]
default = ["popup-winui"]
popup-winui = []
popup-winui-gdi = []

[target.'cfg(windows)'.dependencies]
windows-reactor = { git = "https://github.com/microsoft/windows-rs", rev = "884c9bbc1bd0a2315f00e0f04e34f6b1714653b9", package = "windows-reactor" }
# 若 monorepo 使用 path 依赖写法，按 cargo 报错改为：
# windows-reactor = { git = "...", rev = "...", path 不可跨 git — 使用 package 名即可 }

[target.'cfg(windows)'.build-dependencies]
windows-reactor-setup = { git = "https://github.com/microsoft/windows-rs", rev = "884c9bbc1bd0a2315f00e0f04e34f6b1714653b9", package = "windows-reactor-setup" }
```

> 执行时若 `package`/`rev` 解析失败：打开 monorepo `crates/libs/reactor/Cargo.toml` 确认包名，必要时用 `[patch]` 或 workspace 文档推荐写法。

`build.rs`：

```rust
fn main() {
    tauri_build::build();
    #[cfg(all(windows, feature = "popup-winui"))]
    {
        // framework-dependent：与 v1 发布模型一致
        windows_reactor_setup::as_framework_dependent();
    }
}
```

同步升级 `windows` 版本：以 `cargo check` 报错为准，合并 reactor 所需 features，**保留**现有 OCR/截图 features 列表并补齐缺失项。

- [ ] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui reactor_crate_is_linked -- --nocapture
cargo check -p shizi
```

预期：PASS / 无 error。若 Runtime 相关仅在链接期报错，记入 spike 文档并在任务 2 处理。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs \
  src-tauri/src/app/popup_backend/winui/reactor/mod.rs \
  docs/agent/spike-2026-07-24-winui-reactor-tauri.md
git commit -m "chore(popup-winui): 接入 windows-reactor 依赖骨架（M0）"
```

---

## 任务 2：M0 共存 spike + 否决门记录（**硬门禁**）

**文件：**
- 创建/修改：`src-tauri/src/app/popup_backend/winui/reactor/host.rs`（最小）
- 修改：`src-tauri/src/app/popup_backend/winui/bootstrap.rs`（可先 spike 内部调用）
- 修改：`docs/agent/spike-2026-07-24-winui-reactor-tauri.md`
- 修改：本计划文末「M0 结论」表（同一 commit 或紧随 commit）

- [ ] **步骤 1：实现最小 STA host API（无完整五区）**

```rust
// reactor/host.rs — 接口形状（实现以 M0 实测为准）
use std::sync::mpsc::{self, Sender};
use std::thread;

pub enum HostCmd {
    Show,
    Hide,
    SetLabel(String),
    Shutdown,
}

pub struct ReactorHostHandle {
    tx: Sender<HostCmd>,
}

impl ReactorHostHandle {
    /// 启动 STA 线程；失败返回 Err（Runtime 缺失等）→ 上层降级 WebView
    pub fn start() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<HostCmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        thread::Builder::new()
            .name("shizi-reactor-ui".into())
            .spawn(move || {
                // 1) 设 STA（CoInitializeEx 或 reactor 文档要求的方式）
                // 2) windows_reactor::bootstrap()
                // 3) 哨兵 App / 主循环 + 可 show 的弹窗
                // 4) ready_tx.send(Ok(())) 或 Err
                // 5) 循环处理 rx：Show/Hide/SetLabel
                let _ = ready_tx.send(Err("M0: 在此填入真实实现".into()));
                let _ = rx;
            })
            .map_err(|e| e.to_string())?;

        ready_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| "reactor UI 线程启动超时".to_string())??;

        Ok(Self { tx })
    }

    pub fn publish_label(&self, s: impl Into<String>) {
        let _ = self.tx.send(HostCmd::SetLabel(s.into()));
    }

    pub fn show(&self) {
        let _ = self.tx.send(HostCmd::Show);
    }

    pub fn hide(&self) {
        let _ = self.tx.send(HostCmd::Hide);
    }
}
```

最小 UI（在 STA 线程内，伪代码对齐官方 counter）：

```rust
use windows_reactor::*;

fn spike_popup(cx: &mut RenderCx) -> Element {
    let (label, set_label) = cx.use_state(String::from("spike"));
    // 实际应从共享 slot 读 HostCmd::SetLabel 写入的值
    vstack((
        text_block(label.clone()).font_size(20.0),
        button("Close").on_click(|| { /* hide window, 不退出进程 */ }),
    ))
    .spacing(8.0)
    .into()
}
```

窗口：优先 `App::new().title("Shizi Spike").inner_size(468.0, 320.0).backdrop(Backdrop::Mica).render(..)` **或** 哨兵 + `ReactorWindow`；**必须**验证 hide 后进程仍在。

- [ ] **步骤 2：手动验收清单（全部勾选才算 M0 通过）**

在 `docs/agent/spike-2026-07-24-winui-reactor-tauri.md` 记录结果：

| # | 标准 | 通过? |
|---|------|-------|
| 1 | 同进程弹出真 WinUI 窗（Inspect 可见系统控件 / 非 GDI 矩形） | |
| 2 | Mica 或明确记录 API 不可用原因 | |
| 3 | `SetLabel`/`publish` 后文本可见更新 | |
| 4 | hide → show 稳定，不重建 Runtime | |
| 5 | 关闭弹窗（hide）**不**退出托盘进程 | |
| 6 | 打开设置 WebView 后再 show 弹窗无死锁 | |
| 7 | Runtime 缺失或 bootstrap 失败时返回 Err（可被 `create_host_with_winui_fallback` 降级） | |
| 8 | 写死共存模型 **S1 或 S2** + 精确 git rev | |

timebox：**≤ 1 个有效工作日**。

- [ ] **步骤 3：否决分支**

若表中关键项（1、3、4、5、7）失败：

1. 在 spike 文档写明失败原因与尝试过的 S1/S2。  
2. **停止**任务 3–12 的路径 R UI 对齐。  
3. Commit 仅含 spike 记录 + 依赖回滚或保留实验 feature。  
4. 产品仍默认 `webview`；不把 GDI 重新标成「真 WinUI」。

- [ ] **步骤 4：通过则锁定版本并 Commit**

```bash
# 将 Cargo.toml rev 改为实测绿的 SHA
git add src-tauri/Cargo.toml src-tauri/Cargo.lock \
  src-tauri/src/app/popup_backend/winui/reactor/ \
  src-tauri/src/app/popup_backend/winui/bootstrap.rs \
  docs/agent/spike-2026-07-24-winui-reactor-tauri.md \
  docs/superpowers/plans/2026-07-24-winui-reactor-popup.md
git commit -m "feat(popup-winui): M0 spike 锁定 reactor 与 Tauri 共存模型"
```

**只有本任务标记通过后，才允许进入任务 3。**

---

## 任务 3：状态桥与纯函数（TDD，无窗口）

**文件：**
- 创建：`src-tauri/src/app/popup_backend/winui/reactor/state.rs`
- 创建：`src-tauri/src/app/popup_backend/winui/reactor/meta.rs`
- 创建：`src-tauri/src/app/popup_backend/winui/reactor/langs.rs`
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/mod.rs`

- [ ] **步骤 1：编写失败的测试**

```rust
// 放在 state.rs 的 #[cfg(test)]
use crate::app::popup_backend::types::{PopupCardStatus, PopupCardVm, PopupViewModel};

#[test]
fn resolve_copy_prefers_card_text() {
    let snap = PopupViewModel {
        cards: vec![
            PopupCardVm {
                service_instance_id: "a".into(),
                service_name: "A".into(),
                service_type: "llm".into(),
                protocol: "mock".into(),
                model_name: "m".into(),
                status: PopupCardStatus::Finished,
                text: String::new(),
                error_message: String::new(),
                usage_input: None,
                usage_output: None,
                detected_source_lang: None,
            },
            PopupCardVm {
                service_instance_id: "b".into(),
                service_name: "B".into(),
                service_type: "llm".into(),
                protocol: "openai_chat".into(),
                model_name: "gpt".into(),
                status: PopupCardStatus::Finished,
                text: "你好".into(),
                error_message: String::new(),
                usage_input: Some(1),
                usage_output: Some(2),
                detected_source_lang: None,
            },
        ],
        ..Default::default()
    };
    assert_eq!(resolve_copy_text(&snap, "b").as_deref(), Some("你好"));
    assert_eq!(resolve_copy_text(&snap, "a"), None);
}

#[test]
fn resolve_copy_falls_back_to_error_message() {
    let snap = PopupViewModel {
        cards: vec![PopupCardVm {
            service_instance_id: "e".into(),
            service_name: "E".into(),
            service_type: "llm".into(),
            protocol: "mock".into(),
            model_name: "m".into(),
            status: PopupCardStatus::Failed,
            text: String::new(),
            error_message: "超时".into(),
            usage_input: None,
            usage_output: None,
            detected_source_lang: None,
        }],
        ..Default::default()
    };
    assert_eq!(resolve_copy_text(&snap, "e").as_deref(), Some("超时"));
}
```

`meta.rs` 测试：

```rust
#[test]
fn mt_protocol_hides_model_and_tokens() {
    assert!(is_machine_translate_protocol("microsoft_edge"));
    assert_eq!(display_model_name("microsoft_edge", "anything"), "");
    assert!(!should_show_tokens("microsoft_edge", true));
}

#[test]
fn llm_shows_model_and_tokens_when_usage() {
    assert_eq!(display_model_name("openai_chat", "gpt-4o"), "gpt-4o");
    assert!(should_show_tokens("openai_chat", true));
    assert!(!should_show_tokens("openai_chat", false));
}
```

`langs.rs` 测试：

```rust
#[test]
fn swap_auto_keeps_auto_on_source() {
    let (s, t) = swap_session_langs("auto", "zh-CN");
    // 与现网 GDI ui.rs 行为一致：保持可交换语义；以抽出函数的现实现为准写断言
    assert!(!s.is_empty());
    assert!(!t.is_empty());
}

#[test]
fn lang_display_zh_cn() {
    assert_eq!(lang_display_name("zh-CN"), "简体中文");
}
```

- [ ] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui resolve_copy_ -- --nocapture
```

预期：FAIL（函数未定义）。

- [ ] **步骤 3：实现最少代码**

从 `ui.rs` **搬迁**（勿复制后留两套漂移）逻辑：

```rust
// state.rs
use std::sync::{Arc, Mutex};
use crate::app::popup_backend::types::PopupViewModel;

#[derive(Clone, Default)]
pub struct SharedPopupState {
    inner: Arc<Mutex<PopupViewModel>>,
}

impl SharedPopupState {
    pub fn store(&self, vm: &PopupViewModel) {
        if let Ok(mut g) = self.inner.lock() {
            *g = vm.clone();
        }
    }

    pub fn load(&self) -> PopupViewModel {
        self.inner.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

pub fn resolve_copy_text(vm: &PopupViewModel, service_instance_id: &str) -> Option<String> {
    let card = vm.cards.iter().find(|c| c.service_instance_id == service_instance_id)?;
    let t = card.text.trim();
    if !t.is_empty() {
        return Some(t.to_string());
    }
    let e = card.error_message.trim();
    if !e.is_empty() {
        return Some(e.to_string());
    }
    None
}

pub fn first_copyable_service_id(vm: &PopupViewModel) -> Option<String> {
    vm.cards.iter().find_map(|c| {
        let t = c.text.trim();
        if !t.is_empty() {
            Some(c.service_instance_id.clone())
        } else {
            None
        }
    })
}
```

```rust
// meta.rs
pub fn is_machine_translate_protocol(protocol: &str) -> bool {
    protocol.trim() == "microsoft_edge"
}

pub fn display_model_name(protocol: &str, model_name: &str) -> String {
    if is_machine_translate_protocol(protocol) {
        return String::new();
    }
    let m = model_name.trim();
    if m.is_empty() || m == "—" || m == "-" {
        String::new()
    } else {
        m.to_string()
    }
}

pub fn should_show_tokens(protocol: &str, has_usage: bool) -> bool {
    if is_machine_translate_protocol(protocol) {
        return false;
    }
    has_usage
}
```

```rust
// langs.rs — 自 ui.rs LANG_TABLE / lang_display_name / swap_session_langs 原样迁入
pub const LANG_TABLE: &[(&str, &str)] = &[
    ("auto", "自动检测"),
    ("zh-CN", "简体中文"),
    // … 与 ui.rs 现表保持一致，一次剪贴完整表
];

pub fn lang_display_name(code: &str) -> String { /* … */ }
pub fn swap_session_langs(source: &str, target: &str) -> (String, String) { /* … */ }
```

- [ ] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui resolve_copy_ -- --nocapture
cargo test -p shizi --features popup-winui mt_protocol_ -- --nocapture
cargo test -p shizi --features popup-winui lang_display_ -- --nocapture
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/reactor/
git commit -m "feat(popup-winui): 路径 R 状态桥与卡片/语言纯函数"
```

---

## 任务 4：`actions` 与状态桥解耦 GDI

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/actions.rs`
- 修改：`src-tauri/src/app/popup_backend/winui/mod.rs`（导出 state）

- [ ] **步骤 1：编写失败的测试调整**

将 `actions.rs` 内依赖 `ui::{load_paint_snapshot, resolve_copy_text, PaintSnapshot}` 的单元测试改为依赖 `reactor::state`：

```rust
use crate::app::popup_backend::winui::reactor::state::{
    first_copyable_service_id, resolve_copy_text,
};
// 用 PopupViewModel 代替 PaintSnapshot 构造 sample
```

- [ ] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui actions:: -- --nocapture
```

预期：编译失败（旧 import）。

- [ ] **步骤 3：改 `copy_card_text`**

```rust
fn copy_card_text(service_instance_id: &str) -> Result<(), String> {
    let snap = super::reactor::state::global_snapshot(); // 或 SharedPopupState 静态
    let text = super::reactor::state::resolve_copy_text(&snap, service_instance_id)
        .ok_or_else(|| "没有可复制的译文".to_string())?;
    write_clipboard_text(&text).map_err(|e| e.to_string())
}
```

`handle_user_action_with` 其余分支 **保持不变**（Close/Cancel/Retry/OpenSettings/SetSessionLanguages）。

`install_action_handler`：若 GDI 仍存在，可保留 `ui::set_action_handler`；路径 R 在 `view` 内直接调 `handle_user_action`，可同时：

```rust
pub fn install_action_handler() {
    #[cfg(feature = "popup-winui-gdi")]
    super::ui::set_action_handler(handle_user_action);
}
```

- [ ] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui -- --nocapture
```

预期：相关 tests PASS；全量 default features 无回归。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/actions.rs \
  src-tauri/src/app/popup_backend/winui/mod.rs \
  src-tauri/src/app/popup_backend/winui/reactor/
git commit -m "refactor(popup-winui): 动作复制改为 reactor 状态快照"
```

---

## 任务 5：Reactor host 生产化（ensure/show/hide/publish）

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/host.rs`
- 修改：`src-tauri/src/app/popup_backend/winui/bootstrap.rs`
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/mod.rs`

- [ ] **步骤 1：先写 bootstrap 单测期望变更**

```rust
// bootstrap.rs tests — 替换路径 B 文案
#[test]
fn try_bootstrap_reports_reactor_path() {
    let status = try_bootstrap();
    // Runtime 已装：ok == true 且 message 含 "Reactor" 或 "WinAppSDK"
    // Runtime 未装：ok == false 且 message 非空（CI 若无 Runtime 允许 ok false，但不 panic）
    assert!(!status.message.is_empty());
}
```

- [ ] **步骤 2：运行测试观察当前失败**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui try_bootstrap -- --nocapture
```

预期：旧断言「路径 B」失败或语义不符。

- [ ] **步骤 3：实现 bootstrap + host 队列**

```rust
// bootstrap.rs
pub fn try_bootstrap() -> BootstrapStatus {
    // 调用 windows_reactor::bootstrap() 或 M0 锁定的等价 API
    // 成功：
    // BootstrapStatus { ok: true, message: "路径 R：windows-reactor / WinAppSDK".into() }
    // 失败：
    // BootstrapStatus { ok: false, message: format!("WinAppSDK bootstrap 失败: {e}") }
}
```

`host.rs` 扩展命令：

```rust
pub enum HostCmd {
    Ensure, // 若窗未建则建
    Show(PopupPositionMode),
    Hide,
    Destroy,
    Publish(PopupViewModel),
    Shutdown,
}
```

定位：`NearCursor` 时用现有 `compute_popup_position` + 平台 cursor（与 GDI `show_popup` 同输入）；Reactor 窗 API 用 M0 确认的 move/set bounds。

`publish`：**只** `SharedPopupState::store` + 向 UI 线程 post「request_rerender」；禁止在调用线程阻塞。

- [ ] **步骤 4：无 GUI 的单元测试（队列非阻塞）**

```rust
#[test]
fn publish_does_not_require_window() {
    // host 未 start 时 store 全局快照仍成功（与现 backend publish 窗未创建分支一致）
    let st = SharedPopupState::default();
    let vm = PopupViewModel {
        source_text: "hi".into(),
        ..Default::default()
    };
    st.store(&vm);
    assert_eq!(st.load().source_text, "hi");
}
```

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui publish_does_not_require_window -- --nocapture
cargo test -p shizi --features popup-winui try_bootstrap -- --nocapture
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/reactor/host.rs \
  src-tauri/src/app/popup_backend/winui/bootstrap.rs \
  src-tauri/src/app/popup_backend/winui/reactor/
git commit -m "feat(popup-winui): Reactor host 生命周期与 bootstrap 路径 R"
```

---

## 任务 6：`WinuiPopupBackend` 切换到路径 R + 最小 UI

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/backend.rs`
- 创建：`src-tauri/src/app/popup_backend/winui/reactor/view.rs`（最小：标题关闭 + 源文 + 单卡 + 复制）
- 修改：`src-tauri/src/app/popup_backend/winui/mod.rs`

- [ ] **步骤 1：backend 行为测试（无 HWND）**

现有 host mock 测试应仍过。新增文档化手动用例（写入 spike 或 architecture 附录）：

1. `popupUiBackend=winui` 冷启动  
2. 划词 → 见源文与结果流式  
3. 复制 / 关闭 hide / 再开  

- [ ] **步骤 2：改写 `backend.rs`**

```rust
//! WinuiPopupBackend：路径 R（windows-reactor）

use super::actions;
use super::bootstrap;
use super::reactor::host::ReactorHostHandle;
use crate::app::popup_backend::trait_api::PopupBackend;
use crate::app::popup_backend::types::{PopupPositionMode, PopupUiBackendKind, PopupViewModel};

pub struct WinuiPopupBackend {
    app: tauri::AppHandle,
    host: Option<ReactorHostHandle>,
}

impl WinuiPopupBackend {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app, host: None }
    }

    fn bind_app_for_ui(&self) {
        actions::install_action_handler();
        actions::bind_app(self.app.clone());
    }
}

impl PopupBackend for WinuiPopupBackend {
    fn kind(&self) -> PopupUiBackendKind {
        PopupUiBackendKind::Winui
    }

    fn ensure_created(&mut self) -> Result<(), String> {
        if self.host.as_ref().is_some_and(|h| h.is_alive()) {
            self.bind_app_for_ui();
            return Ok(());
        }
        self.host = None;
        let status = bootstrap::try_bootstrap();
        if !status.ok {
            return Err(status.message);
        }
        self.bind_app_for_ui();
        let handle = ReactorHostHandle::start()?;
        self.host = Some(handle);
        Ok(())
    }

    fn show(&mut self, mode: PopupPositionMode) -> Result<(), String> {
        self.ensure_created()?;
        self.host
            .as_ref()
            .ok_or_else(|| "Reactor 弹窗未创建".to_string())?
            .show(mode)
    }

    fn hide(&mut self) {
        if let Some(h) = self.host.as_ref() {
            h.hide();
        }
    }

    fn destroy(&mut self) {
        if let Some(h) = self.host.take() {
            h.shutdown();
        }
    }

    fn is_visible(&self) -> bool {
        self.host.as_ref().is_some_and(|h| h.is_visible())
    }

    fn is_alive(&self) -> bool {
        self.host.as_ref().is_some_and(|h| h.is_alive())
    }

    fn publish(&mut self, vm: &PopupViewModel) {
        if let Some(h) = self.host.as_ref() {
            h.publish(vm);
        } else {
            super::reactor::state::store_global(vm);
        }
    }
}
```

- [ ] **步骤 3：最小 `view.rs`**

```rust
use windows_reactor::*;
use crate::app::popup_backend::types::{PopupCardStatus, PopupUserAction};
use super::state::SharedPopupState;
// 通过 cx / context 注入 SharedPopupState + action callback

pub fn render_popup(cx: &mut RenderCx) -> Element {
    let vm = /* use_context 或 poll SharedPopupState */;
    let source = vm.source_text.clone();
    let card = vm.cards.first();
    let body = card.map(|c| c.text.clone()).unwrap_or_default();
    let sid = card.map(|c| c.service_instance_id.clone()).unwrap_or_default();

    vstack((
        hstack((
            text_block("柿子翻译").bold(),
            button("关闭").on_click(|| {
                crate::app::popup_backend::winui::actions::handle_user_action(
                    PopupUserAction::Close,
                );
            }),
        ))
        .spacing(8.0),
        text_block(source).wrap().selectable(),
        text_block(body).wrap().selectable(),
        button("复制").on_click(move || {
            crate::app::popup_backend::winui::actions::handle_user_action(
                PopupUserAction::CopyResult {
                    service_instance_id: sid.clone(),
                },
            );
        }),
    ))
    .spacing(12.0)
    .width(468.0)
    .into()
}
```

宽度 **468**；状态文案可用 `text_block` 显示 `is_translating` / `Failed`。

- [ ] **步骤 4：编译与手动冒烟**

```powershell
cd src-tauri
cargo test -p shizi
cargo build -p shizi
# 开发：配置 popupUiBackend=winui 后 npm run tauri dev，划词一次
```

预期：真 WinUI 窗显示源文与结果；关闭 hide；失败 Runtime 降级 WebView dialog。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/
git commit -m "feat(popup-winui): WinuiPopupBackend 切换为路径 R 最小 UI"
```

---

## 任务 7：五区布局 — 标题栏 + 源文 + 状态栏

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/view.rs`

- [ ] **步骤 1：对照 Open Design 清单（无代码测试，勾选实现）**

视觉 SSOT：`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi` → `#popup-winui3` / `src/popup/winui3/*`。

标题栏控件：品牌、钉/收藏/截图/书签（**可 stub** `log::debug`）、设置（`OpenSettings`）、最小化（hide）、关闭（`Close`）；拖动区域用 WinUI 标题栏拖拽 API 或 AppWindow 拖拽（M0/M3 确认）。

- [ ] **步骤 2：实现标题栏 + 源文区 + 状态栏 Element**

```rust
// 状态栏示例
fn status_bar(vm: &PopupViewModel) -> Element {
    let status = if vm.is_translating {
        "翻译中…"
    } else if vm.source_text.trim().is_empty() {
        "就绪"
    } else {
        "完成"
    };
    let count = vm.source_text.chars().count();
    hstack((
        text_block(status.to_string()).caption(),
        text_block(format!("{count} 字")).caption(),
    ))
    .spacing(8.0)
    .into()
}
```

源文：只读 + selectable（任务澄清 §3）。

- [ ] **步骤 3：手动验证**

设置入口打开设置窗；关闭 hide；字数随 `publish` 更新。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/reactor/view.rs
git commit -m "feat(popup-winui): Reactor 弹窗标题栏/源文/状态栏"
```

---

## 任务 8：语言栏（ComboBox / 交换）

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/view.rs`
- 使用：`reactor/langs.rs`

- [ ] **步骤 1：编写纯函数测试（若 swap 尚未覆盖）**

```rust
#[test]
fn swap_exchanges_concrete_langs() {
    let (s, t) = swap_session_langs("en", "zh-CN");
    assert_eq!(s, "zh-CN");
    assert_eq!(t, "en");
}
```

- [ ] **步骤 2：运行测试**

```powershell
cd src-tauri
cargo test -p shizi --features popup-winui swap_exchanges -- --nocapture
```

- [ ] **步骤 3：UI 接线**

使用 `combo_box` 或 `button`+`menu_flyout` 列出 `LANG_TABLE`：

```rust
button(lang_display_name(&vm.source_lang)).on_click(|| { /* open flyout */ });
button("⇄").on_click(|| {
    let (s, t) = swap_session_langs(&src, &tgt);
    actions::handle_user_action(PopupUserAction::SetSessionLanguages {
        source_lang: s,
        target_lang: t,
    });
});
```

选择语言 → `SetSessionLanguages`（自动重译由 `actions` 现有逻辑完成）。

- [ ] **步骤 4：手动验证**

换目标语言触发重译；交换按钮正确。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/reactor/
git commit -m "feat(popup-winui): Reactor 语言栏与会话语言动作"
```

---

## 任务 9：多服务结果列表 + 取消/重试

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/view.rs`
- 使用：`reactor/meta.rs`

- [ ] **步骤 1：卡片渲染函数**

```rust
fn result_card(card: &PopupCardVm) -> Element {
    let status_label = match card.status {
        PopupCardStatus::Pending => "等待中",
        PopupCardStatus::Translating => "翻译中",
        PopupCardStatus::Finished => "",
        PopupCardStatus::Failed => "失败",
        PopupCardStatus::Cancelled => "已取消",
    };
    let model = display_model_name(&card.protocol, &card.model_name);
    let tokens = if should_show_tokens(
        &card.protocol,
        card.usage_input.is_some() || card.usage_output.is_some(),
    ) {
        format!(
            "↑{} ↓{}",
            card.usage_input.unwrap_or(0),
            card.usage_output.unwrap_or(0)
        )
    } else {
        String::new()
    };
    let sid = card.service_instance_id.clone();
    vstack((
        hstack((
            text_block(card.service_name.clone()).semibold(),
            text_block(status_label.to_string()).caption(),
        )),
        text_block(if card.text.is_empty() {
            card.error_message.clone()
        } else {
            card.text.clone()
        })
        .wrap()
        .selectable(),
        hstack((
            text_block(model).caption(),
            text_block(tokens).caption(),
            button("复制").on_click(move || {
                actions::handle_user_action(PopupUserAction::CopyResult {
                    service_instance_id: sid.clone(),
                });
            }),
        )),
    ))
    .spacing(6.0)
    .into()
}
```

列表：`scroll_viewer` + `vstack` 映射 `vm.cards`（保序）。

状态区按钮：

```rust
if vm.is_translating {
    button("取消").on_click(|| {
        actions::handle_user_action(PopupUserAction::CancelTranslation);
    })
} else {
    button("重试").on_click(|| {
        actions::handle_user_action(PopupUserAction::Retry {
            service_instance_id: None,
        });
    })
}
```

- [ ] **步骤 2：手动主路径验收**

多服务并发 → 多卡；单卡失败不影响其他；取消/重试可用。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/reactor/view.rs
git commit -m "feat(popup-winui): Reactor 多服务结果卡与取消重试"
```

---

## 任务 10：视觉抛光（Mica / accent / 滚动）

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/host.rs`（backdrop）
- 修改：`src-tauri/src/app/popup_backend/winui/reactor/view.rs`

- [ ] **步骤 1：窗口属性**

```rust
// 创建窗口时
.inner_size(468.0, 520.0) // 高度可随后续内容调整
.backdrop(Backdrop::Mica) // 若枚举名不同，以 windows-reactor 为准
```

Accent：**#D55A1F** 柿子橙——按钮 `.accent()` 或资源色；深色模式按系统。

- [ ] **步骤 2：滚动与高度**

结果区 `scroll_viewer`；总高上限约工作区 80%（与现网一致，用 `compute_popup_position` 的工作区信息）。

- [ ] **步骤 3：目视对照清单**

| 项 | 期望 |
|----|------|
| 宽度 | ~468 |
| 背景 | Mica 或 Fluent 实底 fallback |
| Accent | 柿子橙可见于主按钮/品牌点 |
| 多卡 | 间距/分割清晰 |
| 非 GDI | Inspect 为 XAML 控件 |

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/reactor/
git commit -m "feat(popup-winui): Reactor 弹窗 Mica/accent 与滚动抛光"
```

---

## 任务 11：GDI 隔离 / 删除 + 模块注释

**文件：**
- 修改：`src-tauri/src/app/popup_backend/winui/ui.rs` → `#[cfg(feature = "popup-winui-gdi")]` 或移至 `winui/legacy/ui_gdi.rs`
- 修改：`src-tauri/src/app/popup_backend/winui/mod.rs`
- 修改：`src-tauri/src/app/popup_backend/mod.rs` 模块文档
- 修改：`src-tauri/Cargo.toml` features 注释

- [ ] **步骤 1：确认 default 构建不编译 GDI 大文件**

```powershell
cd src-tauri
cargo build -p shizi
# 确认 ui.rs 的 GDI 符号不在默认路径引用
```

- [ ] **步骤 2：删除或 cfg 隔离**

推荐：**默认删除** GDI 绘制实现（git 历史可恢复），仅当团队需要对照时保留 `popup-winui-gdi`。若删除：

- 移除 `mod ui` 及 backend 对 `NativePopupHwnd` 的一切引用（应已无）。  
- 删除仅服务 GDI 的测试。

- [ ] **步骤 3：全量测试**

```powershell
cd src-tauri
cargo test -p shizi
cargo test -p shizi --no-default-features
```

预期：PASS。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/popup_backend/winui/ src-tauri/Cargo.toml
git commit -m "refactor(popup-winui): 移除 GDI 路径 B，winui 仅路径 R"
```

---

## 任务 12：CI / 文档收尾

**文件：**
- 修改：`.github/workflows/ci.yml`
- 修改：`docs/agent/architecture-notes.md`
- 修改：`README.md`
- 修改：`AGENTS.md` 与 `CLAUDE.md`（同步）
- 修改：`docs/superpowers/specs/2026-07-24-winui-reactor-popup-design.md`（验收勾选，若有）
- 修改：`docs/superpowers/specs/2026-07-24-winui-popup-backend-design.md`（交叉引用：实现改为路径 R）

- [ ] **步骤 1：CI**

Windows job：

- `cargo test` / `cargo build` 使用 default features（含 `popup-winui`）  
- 安装 Windows App Runtime（与 M0 pin 版本一致）或文档说明 framework-dependent 下 CI 仅编译、GUI 自测在本地  

示例步骤（按 runner 调整）：

```yaml
- name: Install Windows App Runtime
  shell: pwsh
  run: |
    # 使用微软官方安装包 URL（与 spike 文档版本一致）
    # winget 或 Invoke-WebRequest + 静默安装
    Write-Host "按 docs/agent/spike-2026-07-24-winui-reactor-tauri.md 安装 Runtime"
```

- [ ] **步骤 2：architecture-notes**

写明：

- `popupUiBackend=winui` → **路径 R：windows-reactor（真 WinUI 3）**  
- 共存模型 S1/S2（抄 spike 结论）  
- 失败降级 WebView  
- 开发依赖 Runtime；framework-dependent  

- [ ] **步骤 3：README 开发小节**

增加：安装 Windows App Runtime；`popupUiBackend` 说明。

- [ ] **步骤 4：验证命令**

```powershell
cd src-tauri
cargo test -p shizi
cd ..
npm run typecheck
```

- [ ] **步骤 5：Commit**

```bash
git add .github/workflows/ci.yml docs/ README.md AGENTS.md Claude.md \
  docs/agent/architecture-notes.md docs/superpowers/specs/
git commit -m "docs(popup-winui): 路径 R 架构说明与 CI/Runtime 依赖"
```

---

## 手动端到端验收（编码阶段结束前）

| # | 步骤 | 期望 |
|---|------|------|
| 1 | 配置 `winui`，已装 Runtime，重启 | 弹窗为真 WinUI 3 |
| 2 | Alt+D 划词 | 源文 + 多卡流式 |
| 3 | 换语言 | 重译 |
| 4 | 复制 | 剪贴板正确 |
| 5 | 设置按钮 | 打开设置 WebView |
| 6 | 关闭 | hide，托盘仍在 |
| 7 | 再开 | 位置/状态正常 |
| 8 | 卸载/破坏 Runtime 模拟失败 | 降级 WebView + dialog |
| 9 | `cargo test` default | 全绿 |

---

## 计划自检（对照 spec）

| Spec 章节 | 对应任务 |
|-----------|----------|
| 目标 1–6 真 WinUI / 契约 / 视觉 / 降级 / 无 .NET / 纯 Rust | 1–12 整体 |
| 非目标 v1 | 「本轮明确不做」 |
| S1/S2/S3 共存 | 任务 2 否决门 |
| 模块落点 reactor/ | 文件结构 + 任务 3–6 |
| 路径 B 处置 | 任务 11 |
| 必接 PopupUserAction | 任务 6–9 |
| 源文策略 | 澄清 §3 + 任务 7 |
| framework-dependent | 澄清 §4 + 任务 1 build.rs |
| 测试策略 | 任务 3 单测 + 手动表 |
| M0–M4 分期 | 里程碑映射 |
| 成功标准 | 端到端验收表 |

**占位符扫描：** 无「TODO/待定」实现步骤；版本 pin 明确要求 M0 写入具体 SHA。  
**类型一致性：** `PopupViewModel` / `PopupUserAction` / `PopupBackend` 与现 `types.rs`/`trait_api.rs` 一致；不新增配置枚举值。

---

## M0 结论（执行任务 2 后填写）

| 项 | 结论 |
|----|------|
| 共存模型 | **S1：同进程 + 专用 STA 线程**（`shizi-reactor-ui`）；否决门 **Go** |
| `windows-rs` git rev | `884c9bbc1bd0a2315f00e0f04e34f6b1714653b9`（已锁定） |
| `windows` crate 版本 | 应用侧 **0.58**；reactor 侧 `windows-core 0.62.2` 并存（M0 未强制统一升级） |
| 哨兵窗 / last-window-exit 处理 | 主 `App` 窗为哨兵（`Shizi Reactor Sentinel`，1×1，立即 hide，永不 Close）；弹窗 `ReactorWindow` 仅 `ShowWindow` hide/show；reactor 在最后一扇已注册窗 Closed 时 `process::exit(0)` |
| Runtime 版本 / 安装备注 | framework-dependent；reactor-setup 对齐 WASDK Runtime **2.3.1**；本机已装 App Runtime 2.x，bootstrap 成功；缺失时 `start()`/`try_bootstrap` → Err 可降级 WebView |
| 否决时的产品策略 | **未否决**；若后续回归失败：停路径 R UI、保留 WebView 默认、GDI 不得冒充真 WinUI |

---

## 风险与执行注意

1. **先 M0 后 UI**：违反则可能整周浪费在无法共存的控件上。  
2. **git 依赖构建慢**：CI/本机首次拉 monorepo 可能很大；锁定 rev 避免漂。  
3. **`windows` 升级**：OCR/截图/PDF 回归测试必须跑。  
4. **release `panic`**：reactor 文档建议 `panic = "abort"`；评估与 Tauri 现配置冲突后再改，勿在 M0 外擅自全局改 profile。  
5. **禁止**在 UI 层实现翻译协议或写历史库。
