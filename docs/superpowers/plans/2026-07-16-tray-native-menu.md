# 托盘原生菜单样式对齐 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 按方案 3 重排托盘系统原生菜单（划词翻译 / 截图翻译 / 文字识别 → 分隔 → 偏好设置 → 分隔 → 退出 shizi），展示并热更新 `AppConfig.shortcuts` 加速键，托盘可触发截图翻译且与 `Alt+S` / `ShortcutAction::OcrTranslate` 同路径。

**架构：** 继续只用 Tauri `TrayIcon` + 系统 `Menu`/`MenuItem`/`PredefinedMenuItem::separator`。文案本轮**中文硬编码**（不对齐 8 语 locale）。加速键从 config 只读展示，**禁止**在 tray 再 `GlobalShortcut::register`。加速键热更新优先对已有 `MenuItem` 调用 `set_accelerator`（Tauri 2.11.5 / muda 0.19 已支持）；若实机发现系统 accelerator 抢键，切换 `TrayAccelMode::TextOnly`（文案右侧拼接、accelerator 恒 `None`）。

**技术栈：** Rust、Tauri 2.11.x、muda（经 tauri menu 封装）、cargo test

**规格来源：** `docs/superpowers/specs/2026-07-16-tray-native-menu-design.md`

---

## 与 spec 的实现澄清（含用户补充）

1. **本轮不做国际化扩展**  
   - 不改 `frontend/src/i18n/locales/*.json`  
   - 不改 `core/i18n` 内置消息、不迁移 `tray.translate` → 新 key  
   - 菜单五项文案固定为 zh-CN 产品文案（见下表）  
   - `apply_interface_language_locked` 对托盘**菜单项**改为写入同一套中文常量，避免语言切换把「划词翻译」盖回旧「翻译」；tooltip / 窗口标题仍可走现有 messages（本计划不强制改语义）

2. **加速键更新策略：优先原位 `set_accelerator`**  
   - 已核实：`tauri 2.11.5` 的 `MenuItem::set_accelerator(Option<S>)` 存在；内部 `parse().ok()`，非法串会静默变 `None`  
   - 本计划在调用前用自有 `menu_accelerator` + 可选 `accelerator_parse_ok` 做 trim/`log::warn`，再 `set_accelerator`  
   - **不**默认重建整个 `Menu` / `tray.set_menu`；仅当原位 API 在目标环境实测失败时才退到重建（见任务 5 注释与验收）

3. **抢键降级**  
   - 默认 `TrayAccelMode::Native`：右侧系统 accelerator  
   - 降级 `TrayAccelMode::TextOnly`：`MenuItem` accelerator 恒 `None`，文案用 `\t{keys}` 拼接（Win 原生菜单习惯，右侧对齐展示）  
   - 模式用 `tray.rs` 内 `const TRAY_ACCEL_MODE` 常量切换，**不**引入配置项（YAGNI）

4. **截图翻译**  
   - 菜单 id `screenshot` → 调用 `shortcuts::trigger_ocr_translate`（从现有 `handle_global_shortcut` 的 `OcrTranslate` 分支抽出），内部 `spawn` + `start_translation_from_ocr`  
   - tray **不得**复制 DXGI/overlay 编排代码

5. **划词翻译**  
   - 菜单 id 由 `translate` 改为 `selection`；动作仍为「打开翻译弹窗」`show_translation_popup`（与现 tray 行为一致，**不**改为模拟划词 `Ctrl+C`）

6. **退出项**  
   - 无加速键；文案「退出 shizi」；**不**绑定 `Ctrl+Q`

---

## 菜单定稿（实现对照表）

| 顺序 | menu id | 中文文案常量 | 动作 | shortcuts map id |
|---|---|---|---|---|
| 1 | `selection` | `划词翻译` | `show_translation_popup` | `translate-selection` |
| 2 | `screenshot` | `截图翻译` | `trigger_ocr_translate` | `translate-screenshot` |
| 3 | `ocr` | `文字识别` | `show_ocr_window` | `ocr-recognize` |
| — | separator | — | — | — |
| 4 | `settings` | `偏好设置` | `show_settings_window` | `open-settings` |
| — | separator | — | — | — |
| 5 | `quit` | `退出 shizi` | `app.exit(0)` | — |

---

## 文件结构

| 文件 | 职责 |
|---|---|
| 修改 `src-tauri/src/app/tray.rs` | 菜单结构、中文常量、`menu_accelerator` / 表驱动、`TrayI18nHandles` 扩展、`setup_tray`、`refresh_tray_menu_accelerators`、`TrayAccelMode`、单测 |
| 修改 `src-tauri/src/app/shortcuts.rs` | 抽出并 `pub` `trigger_ocr_translate`，`handle_global_shortcut` 复用 |
| 修改 `src-tauri/src/ui/config.rs` | `save_app_config` 在 shortcuts 替换成功后调用 `refresh_tray_menu_accelerators`（best-effort） |
| 修改 `src-tauri/src/ui/i18n.rs` | `TrayI18nHandles` 字段改名后编译通过；托盘菜单 `set_text` 改用 tray 中文常量，不再用 `messages["tray.translate"]` 等盖写 |
| 修改 `README.md`（若有托盘能力描述） | 一句同步菜单信息架构（编码收尾） |
| 规格复选 / 本 plan 复选框 | 执行阶段回填 |

**刻意不改：**  
- 前端 locale JSON、`core/i18n` 字典与相关单测期望  
- WebView 品牌菜单、左键/右键双轨  
- 托盘图标、双击托盘、`pot-desktop`  
- 全局快捷键注册逻辑（除抽出 trigger 函数）  
- `window.settingsTitle` 等窗口标题文案

---

## API 选型记录（实现前已查）

```text
MenuItem::with_id(manager, id, text, enabled, accelerator: Option<&str>)
  → 内部 parse().ok()，非法加速键静默 None

MenuItem::set_accelerator(Option<S>) -> Result
  → 同样 parse().ok() 后原位更新；支持 Some / None 清除

PredefinedMenuItem::separator(manager) -> Result

TrayIcon::set_menu / 重建：本轮默认不用
```

---

### 任务 1：`menu_accelerator` 纯函数 + 表驱动常量（TDD）

**文件：**
- 修改：`src-tauri/src/app/tray.rs`
- 测试：同文件 `#[cfg(test)] mod tests`

- [ ] **步骤 1：编写失败的测试**

在 `tray.rs` 的 `tests` 模块（保留现有 tray 图标测试）追加：

```rust
use super::{menu_accelerator, tray_menu_bindings};
use std::collections::HashMap;

#[test]
fn menu_accelerator_trims_and_skips_empty() {
    let mut map = HashMap::new();
    map.insert("translate-selection".into(), "  Alt+D  ".into());
    map.insert("translate-screenshot".into(), "".into());
    map.insert("ocr-recognize".into(), "   ".into());

    assert_eq!(
        menu_accelerator(&map, "translate-selection").as_deref(),
        Some("Alt+D")
    );
    assert_eq!(menu_accelerator(&map, "translate-screenshot"), None);
    assert_eq!(menu_accelerator(&map, "ocr-recognize"), None);
    assert_eq!(menu_accelerator(&map, "missing-id"), None);
}

#[test]
fn tray_menu_bindings_order_and_shortcut_ids() {
    let rows = tray_menu_bindings();
    let ids: Vec<&str> = rows.iter().map(|r| r.menu_id).collect();
    assert_eq!(ids, ["selection", "screenshot", "ocr", "settings"]);
    assert_eq!(rows[0].shortcut_id, Some("translate-selection"));
    assert_eq!(rows[1].shortcut_id, Some("translate-screenshot"));
    assert_eq!(rows[2].shortcut_id, Some("ocr-recognize"));
    assert_eq!(rows[3].shortcut_id, Some("open-settings"));
    // quit 不在 bindings 表（无加速键）；由 setup 单独追加
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```bash
cd src-tauri && cargo test menu_accelerator_trims_and_skips_empty tray_menu_bindings_order_and_shortcut_ids -- --nocapture
```

预期：FAIL（`menu_accelerator` / `tray_menu_bindings` 未定义）

- [ ] **步骤 3：最少实现让测试通过**

在 `tray.rs` 中（`setup_tray` 之前）增加：

```rust
use std::collections::HashMap;

/// 托盘加速键展示模式。Native = 系统 MenuItem accelerator；TextOnly = 文案拼接、不注册 accelerator。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAccelMode {
    Native,
    TextOnly,
}

/// 默认 Native。若 Windows 实机发现 accelerator 在菜单未打开时抢键，改为 TextOnly。
const TRAY_ACCEL_MODE: TrayAccelMode = TrayAccelMode::Native;

pub const TRAY_LABEL_SELECTION: &str = "划词翻译";
pub const TRAY_LABEL_SCREENSHOT: &str = "截图翻译";
pub const TRAY_LABEL_OCR: &str = "文字识别";
pub const TRAY_LABEL_SETTINGS: &str = "偏好设置";
pub const TRAY_LABEL_QUIT: &str = "退出 shizi";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayMenuBinding {
    pub menu_id: &'static str,
    pub label: &'static str,
    pub shortcut_id: Option<&'static str>,
}

/// 有加速键的菜单项（顺序即菜单上半 + 设置项）。quit 与分隔线由 setup 组装。
pub fn tray_menu_bindings() -> &'static [TrayMenuBinding] {
    &[
        TrayMenuBinding {
            menu_id: "selection",
            label: TRAY_LABEL_SELECTION,
            shortcut_id: Some("translate-selection"),
        },
        TrayMenuBinding {
            menu_id: "screenshot",
            label: TRAY_LABEL_SCREENSHOT,
            shortcut_id: Some("translate-screenshot"),
        },
        TrayMenuBinding {
            menu_id: "ocr",
            label: TRAY_LABEL_OCR,
            shortcut_id: Some("ocr-recognize"),
        },
        TrayMenuBinding {
            menu_id: "settings",
            label: TRAY_LABEL_SETTINGS,
            shortcut_id: Some("open-settings"),
        },
    ]
}

/// 从 shortcuts map 取展示用加速键：缺 key / trim 空 → None；否则 Some(trimmed)。
pub fn menu_accelerator(shortcuts: &HashMap<String, String>, id: &str) -> Option<String> {
    shortcuts
        .get(id)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn menu_item_text(label: &str, accel: Option<&str>) -> String {
    match (TRAY_ACCEL_MODE, accel) {
        (TrayAccelMode::TextOnly, Some(keys)) => format!("{label}\t{keys}"),
        _ => label.to_string(),
    }
}

fn menu_item_accelerator_arg(accel: Option<&str>) -> Option<&str> {
    match TRAY_ACCEL_MODE {
        TrayAccelMode::Native => accel,
        TrayAccelMode::TextOnly => None,
    }
}
```

- [ ] **步骤 4：运行测试确认通过**

```bash
cd src-tauri && cargo test menu_accelerator_trims_and_skips_empty tray_menu_bindings_order_and_shortcut_ids -- --nocapture
```

预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/tray.rs
git commit -m "test(tray): 加速键纯函数与菜单绑定表"
```

---

### 任务 2：重建托盘菜单结构 + 事件 id + `TrayI18nHandles`

**文件：**
- 修改：`src-tauri/src/app/tray.rs`
- 修改：`src-tauri/src/ui/i18n.rs`（字段改名编译通过 + 中文常量 set_text）
- 依赖：任务 1

- [ ] **步骤 1：扩展 `TrayI18nHandles`**

```rust
#[derive(Clone)]
pub struct TrayI18nHandles {
    pub tray: TrayIcon,
    pub selection: MenuItem<tauri::Wry>,
    pub screenshot: MenuItem<tauri::Wry>,
    pub ocr: MenuItem<tauri::Wry>,
    pub settings: MenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
    pub settings_title: Arc<RwLock<String>>,
}
```

删除旧字段 `translate`。

- [ ] **步骤 2：重写 `setup_tray` 菜单组装（加速键可先全 None，任务 4 再接 config）**

要点：

```rust
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

// 读 config（AppState 已在 setup 中 manage）
let shortcuts = app
    .state::<AppState>()
    .config_store
    .get()
    .map(|c| c.shortcuts)
    .unwrap_or_default();

// 对 bindings 逐项 MenuItem::with_id(..., menu_item_accelerator_arg(accel.as_deref()))
// 创建两个 PredefinedMenuItem::separator(app)?
// Menu::with_items(app, &[
//   &selection, &screenshot, &ocr, &sep1, &settings, &sep2, &quit
// ])?

// on_menu_event:
// "selection" => show_translation_popup（同原 translate）
// "screenshot" => 先 log 占位或直接调 trigger（若任务 3 已合入则接真实现）
// "ocr" => show_ocr_window
// "settings" => show_settings_window
// "quit" => app.exit(0)
```

**若任务 3 尚未合并：** `"screenshot"` 分支可暂调 `crate::app::shortcuts::trigger_ocr_translate` 的前置空实现不存在会编译失败——**按顺序先完成任务 3，或本任务与任务 3 同一 commit 落地**。推荐执行顺序：任务 3 → 再完成任务 2 的事件接线（或合并为一个 commit 组）。

推荐原子边界：任务 2 只改结构 + `selection`/`ocr`/`settings`/`quit` 事件；`screenshot` 在任务 3 接线。

- [ ] **步骤 3：修正 `ui/i18n.rs` 编译与文案策略**

将：

```rust
handles.translate.set_text(&messages["tray.translate"])?;
handles.settings.set_text(&messages["tray.settings"])?;
handles.quit.set_text(&messages["tray.quit"])?;
```

改为使用 tray 常量（并补全新项），例如：

```rust
use crate::app::tray::{
    TRAY_LABEL_OCR, TRAY_LABEL_QUIT, TRAY_LABEL_SCREENSHOT, TRAY_LABEL_SELECTION,
    TRAY_LABEL_SETTINGS,
};

handles
    .selection
    .set_text(TRAY_LABEL_SELECTION)
    .map_err(|e| format!("无法更新托盘划词菜单: {e}"))?;
handles
    .screenshot
    .set_text(TRAY_LABEL_SCREENSHOT)
    .map_err(|e| format!("无法更新托盘截图菜单: {e}"))?;
handles
    .ocr
    .set_text(TRAY_LABEL_OCR)
    .map_err(|e| format!("无法更新托盘识别菜单: {e}"))?;
handles
    .settings
    .set_text(TRAY_LABEL_SETTINGS)
    .map_err(|e| format!("无法更新托盘设置菜单: {e}"))?;
handles
    .quit
    .set_text(TRAY_LABEL_QUIT)
    .map_err(|e| format!("无法更新托盘退出菜单: {e}"))?;
// tooltip 仍用 messages["tray.tooltip"]
```

说明：本轮**有意**让界面语言切换不再改托盘菜单项语言，只保留 tooltip / 窗口标题的 i18n。

- [ ] **步骤 4：编译检查**

```bash
cd src-tauri && cargo test tray_ -- --nocapture
cd src-tauri && cargo build
```

预期：编译通过；现有 tray 图标测试 + 任务 1 测试 PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/tray.rs src-tauri/src/ui/i18n.rs
git commit -m "feat(tray): 原生菜单信息架构与分隔线"
```

---

### 任务 3：抽出 `trigger_ocr_translate` 并接线托盘「截图翻译」

**文件：**
- 修改：`src-tauri/src/app/shortcuts.rs`
- 修改：`src-tauri/src/app/tray.rs`（`on_menu_event` 的 `screenshot`）

- [ ] **步骤 1：在 `shortcuts.rs` 增加 pub 入口，并替换内部 match 分支**

```rust
/// 截图 OCR 后进入翻译链路（全局快捷键 Alt+S 与托盘「截图翻译」共用）。
pub fn trigger_ocr_translate(app: &tauri::AppHandle) {
    let state = app.state::<AppState>().inner().clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        start_translation_from_ocr(app_handle, state).await;
    });
}
```

在 `handle_global_shortcut` 中：

```rust
Some(ShortcutAction::OcrTranslate) => trigger_ocr_translate(app),
```

- [ ] **步骤 2：tray 事件**

```rust
"screenshot" => {
    crate::app::shortcuts::trigger_ocr_translate(app);
}
```

- [ ] **步骤 3：编译与单测**

```bash
cd src-tauri && cargo test --lib
cd src-tauri && cargo build
```

预期：PASS / 编译成功（shortcuts 现有 classify 测试不受影响）

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/shortcuts.rs src-tauri/src/app/tray.rs
git commit -m "feat(tray): 截图翻译入口复用 OcrTranslate 路径"
```

---

### 任务 4：加速键应用 + `refresh_tray_menu_accelerators` + save 热更新

**文件：**
- 修改：`src-tauri/src/app/tray.rs`
- 修改：`src-tauri/src/ui/config.rs`
- 测试：`tray.rs` 可增 `menu_item_text` / 模式相关纯函数测试（无需 Tauri App）

- [ ] **步骤 1：编写失败的展示文案测试（TextOnly / Native）**

```rust
#[test]
fn menu_item_text_respects_accel_mode_shape() {
    // 通过测试 menu_item_text 行为；若函数对 TRAY_ACCEL_MODE 依赖 const，
    // 改为纯函数：fn format_tray_label(mode, label, accel) 便于测两种模式。
    assert_eq!(
        format_tray_label(TrayAccelMode::Native, "划词翻译", Some("Alt+D")),
        "划词翻译"
    );
    assert_eq!(
        format_tray_label(TrayAccelMode::TextOnly, "划词翻译", Some("Alt+D")),
        "划词翻译\tAlt+D"
    );
    assert_eq!(
        format_tray_label(TrayAccelMode::TextOnly, "划词翻译", None),
        "划词翻译"
    );
}
```

将任务 1 的 `menu_item_text` 重构为可测的 `format_tray_label(mode, label, accel)`，内部 `TRAY_ACCEL_MODE` 仅作为默认入参。

- [ ] **步骤 2：实现 `apply_accelerators_to_handles`**

```rust
pub fn refresh_tray_menu_accelerators(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let shortcuts = match state.config_store.get() {
        Ok(c) => c.shortcuts,
        Err(e) => {
            log::warn!("刷新托盘加速键失败：无法读配置: {e}");
            return;
        }
    };
    let handles = app.state::<TrayI18nHandles>();
    if let Err(e) = apply_accelerators_to_handles(&handles, &shortcuts) {
        log::warn!("刷新托盘加速键失败: {e}");
    }
}

fn apply_accelerators_to_handles(
    handles: &TrayI18nHandles,
    shortcuts: &HashMap<String, String>,
) -> Result<(), String> {
    let pairs: [(&MenuItem<tauri::Wry>, &str, &str); 4] = [
        (&handles.selection, TRAY_LABEL_SELECTION, "translate-selection"),
        (&handles.screenshot, TRAY_LABEL_SCREENSHOT, "translate-screenshot"),
        (&handles.ocr, TRAY_LABEL_OCR, "ocr-recognize"),
        (&handles.settings, TRAY_LABEL_SETTINGS, "open-settings"),
    ];
    for (item, label, shortcut_id) in pairs {
        let accel = menu_accelerator(shortcuts, shortcut_id);
        // 非法串：Tauri set_accelerator 会 parse 失败变 None；此处主动 warn
        if let Some(ref keys) = accel {
            if keys.parse::<muda::accelerator::Accelerator>().is_err()
                && keys
                    .parse::<muda::accelerator::KeyAccelerator>()
                    .is_err()
            {
                // 若 muda 类型不方便直接依赖，可省略 parse 校验，仅依赖 set 行为 + 注释
                log::warn!("托盘加速键无法解析，将不显示: id={shortcut_id} keys={keys}");
            }
        }
        let text = format_tray_label(TRAY_ACCEL_MODE, label, accel.as_deref());
        item.set_text(text)
            .map_err(|e| format!("set_text {shortcut_id}: {e}"))?;
        let native = match TRAY_ACCEL_MODE {
            TrayAccelMode::Native => accel.as_deref(),
            TrayAccelMode::TextOnly => None,
        };
        item.set_accelerator(native)
            .map_err(|e| format!("set_accelerator {shortcut_id}: {e}"))?;
    }
    // quit：确保无加速键、文案固定
    handles
        .quit
        .set_text(TRAY_LABEL_QUIT)
        .map_err(|e| format!("set_text quit: {e}"))?;
    handles
        .quit
        .set_accelerator(None::<&str>)
        .map_err(|e| format!("set_accelerator quit: {e}"))?;
    Ok(())
}
```

**依赖说明：** 若 `muda` 未在 `Cargo.toml` 直接依赖，**不要**为 parse 单独加依赖——删掉手动 parse 分支，改为：

```rust
// set_accelerator 内部 parse().ok()；非法 keys 静默清除。
// 可选：创建前后对比 text 长度无法判断，保持 log 在空串路径即可。
item.set_accelerator(native)...
```

`setup_tray` 创建 item 时：先用 `menu_accelerator` + `format_tray_label` + `menu_item_accelerator_arg` 填入初始值，与 refresh 同源。

- [ ] **步骤 3：`save_app_config` 挂钩**

在 `src-tauri/src/ui/config.rs` 中，`replace_global_shortcuts` 与 `config_store.save` 成功之后、`emit("app-config:changed")` 前后均可；推荐放在 `apply_interface_language_locked` **之后**（语言路径会 set_text 中文常量，refresh 再写加速键/TextOnly 文案，避免被盖掉）。

```rust
// apply_interface_language_locked(...)?;

crate::app::tray::refresh_tray_menu_accelerators(&app);

app.emit("app-config:changed", &saved_config)...
```

`refresh` 内部已 `log::warn`，**不**把加速键刷新失败升级为 `save_app_config` 的 `Err`。

- [ ] **步骤 4：测试与编译**

```bash
cd src-tauri && cargo test menu_ item_text_respects_accel_mode_shape menu_accelerator -- --nocapture
cd src-tauri && cargo build
```

预期：PASS / 编译成功

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/tray.rs src-tauri/src/ui/config.rs
git commit -m "feat(tray): 加速键展示与配置保存热更新"
```

---

### 任务 5：语言 apply 与加速键一致性 + 抢键降级说明落地

**文件：**
- 修改：`src-tauri/src/ui/i18n.rs`（若任务 2 只 set 纯 label，本任务在 apply 末尾调 `refresh_tray_menu_accelerators`）
- 修改：`src-tauri/src/app/tray.rs`（文件头注释写清降级开关）

- [ ] **步骤 1：`apply_interface_language_locked` 在更新完托盘中文 label / tooltip 后调用**

```rust
// 菜单 set_text 常量之后：
drop(/* 无需 */); // 直接：
crate::app::tray::refresh_tray_menu_accelerators(app);
```

避免：语言切换 set_text 掉 TextOnly 下的 `\tAlt+D` 后缀后加速键展示丢失。

- [ ] **步骤 2：在 `tray.rs` 顶部注释固化选型**

```rust
// 加速键策略：
// 1) 默认 TrayAccelMode::Native + MenuItem::set_accelerator 原位更新（Tauri 2.11+）
// 2) 禁止 tray 内 GlobalShortcut::register（与 shortcuts.rs 双绑）
// 3) 若实机抢键：将 TRAY_ACCEL_MODE 改为 TextOnly（文案 \t 拼接，accelerator 恒 None）
// 4) 不默认 tray.set_menu 重建；仅当 set_accelerator 在目标环境不可用时再考虑重建
```

- [ ] **步骤 3：编译**

```bash
cd src-tauri && cargo test --lib
cd src-tauri && cargo build
```

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/i18n.rs src-tauri/src/app/tray.rs
git commit -m "fix(tray): 语言切换后重刷加速键展示"
```

---

### 任务 6：文档同步与手工验收清单

**文件：**
- 修改：`README.md`（若存在托盘/快捷键能力 bullet，补「托盘菜单：划词/截图/识别/偏好设置/退出 + 加速键展示」）
- 修改：`docs/superpowers/specs/2026-07-16-tray-native-menu-design.md`（状态可改为「实现中/已实现」由执行阶段处理）
- 本 plan 复选框执行时回填

- [ ] **步骤 1：README 一句能力同步（有则改、无则跳过新增大段）**

- [ ] **步骤 2：手工验收（Windows，执行阶段）**

按 spec §9.2：

1. 右键托盘：五项 + 两处分隔，顺序正确  
2. 默认加速键：`Alt+D` / `Alt+S` / `Alt+O` / `Ctrl+,`；退出无  
3. 设置页清空某快捷键并保存 → 对应项加速键消失；改回恢复  
4. 点击五项动作正确；截图翻译与 `Alt+S` 同链路  
5. 切换界面语言 → 托盘**菜单文案保持中文产品文案**（本轮预期）；tooltip 可随语言变  
6. 全局快捷键仍可用；无双绑报错  
7. **抢键抽检**：不打开托盘菜单时按 `Alt+D` 等，确认仍由 global-shortcut 处理、无异常双触发；若系统 accelerator 抢键，将 `TRAY_ACCEL_MODE` 改为 `TextOnly` 并回归 1–6  

- [ ] **步骤 3：Commit**

```bash
git add README.md docs/superpowers/specs/2026-07-16-tray-native-menu-design.md docs/superpowers/plans/2026-07-16-tray-native-menu.md
git commit -m "docs(tray): 同步托盘原生菜单能力说明"
```

---

## 自检（writing-plans）

### 1. 规格覆盖度

| Spec 需求 | 对应任务 |
|---|---|
| 系统原生 Menu，无 WebView 品牌菜单 | 任务 2（范围约束） |
| 方案 3 顺序 + 两处分隔 | 任务 2 |
| 截图翻译 + 保留文字识别 | 任务 2–3 |
| 加速键读 shortcuts，空不显示 | 任务 1、4 |
| 禁止 global-shortcut 双绑 | 任务 3–4 注释 + 不调用 register |
| save 后热更新 | 任务 4 |
| 原位 set_accelerator vs 重建选型 | 任务 4–5 + API 选型记录 |
| 抢键降级 TextOnly | 任务 1 `TrayAccelMode` + 任务 5 注释 + 验收 7 |
| i18n tray.* 8 语 | **本轮按用户指示排除**；任务 2 用中文常量 + i18n apply 最小兼容 |
| 纯函数单测 | 任务 1、4 |
| 错误 best-effort | 任务 4 `refresh` warn |

### 2. 占位符扫描

无「TODO/待定/类似任务 N」；代码块为可粘贴实现骨架。`muda` 直接 parse 校验标为可选并提供不引入新依赖的路径。

### 3. 类型一致性

- menu id：`selection` / `screenshot` / `ocr` / `settings` / `quit`  
- shortcut id：`translate-selection` / `translate-screenshot` / `ocr-recognize` / `open-settings`  
- 句柄字段：`selection`（非 `translate`）  
- 函数名：`menu_accelerator`、`tray_menu_bindings`、`trigger_ocr_translate`、`refresh_tray_menu_accelerators`、`format_tray_label`  
- 模式：`TrayAccelMode::{Native, TextOnly}` + `TRAY_ACCEL_MODE`

### 4. 建议执行顺序

1 → 3（可与 2 紧挨）→ 2 完成事件 → 4 → 5 → 6  

或合并 commit：`1` | `2+3` | `4+5` | `6`（仍保持逻辑原子性）。

---

## 风险与回退

| 风险 | 回退 |
|---|---|
| `set_accelerator` 运行时异常 | `refresh` 已 catch log；可改为重建 menu（本轮不预写重建代码） |
| Native accelerator 抢键 | `TRAY_ACCEL_MODE = TextOnly` |
| 语言切换冲掉 TextOnly 后缀 | 任务 5 apply 后 refresh |
| 与旧 id `translate` 残留 | 全仓事件 id 改为 `selection`，无兼容别名（无对外 API） |
