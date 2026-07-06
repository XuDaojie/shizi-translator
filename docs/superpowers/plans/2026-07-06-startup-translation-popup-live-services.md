# 启动翻译弹窗与服务卡片实时同步 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 应用启动后直接显示翻译弹窗；点击设置按钮打开独立设置窗口；设置页保存服务启用状态后，已打开的翻译弹窗立即同步结果卡片。

**架构：** 让 `main` 窗口承载 `translate.html`，设置页改由独立 `settings` 窗口承载 `settings.html`。后端 `save_app_config` 在保存成功后广播 `app-config:changed`，翻译弹窗监听该事件并复用一个小型纯 JS helper 同步启用服务卡片。保留现有翻译执行链路、卡片样式和 Vue 设置页，不新增依赖。

**技术栈：** Tauri 2 + Rust edition 2021 + 原生静态 `frontend/public/translate.*` + Vue 3 设置页 + Vitest。

**关联文档：**
- spec：[docs/superpowers/specs/2026-07-06-startup-translation-popup-live-services-design.md](../specs/2026-07-06-startup-translation-popup-live-services-design.md)

---

## 文件结构

**新增文件：**

| 文件 | 职责 |
|---|---|
| `frontend/public/translate-card-sync.js` | 翻译弹窗服务卡片同步 helper：筛选启用服务、更新/删除/排序卡片、更新语言显示 |
| `frontend/src/translate-card-sync.test.js` | 直接测试静态页实际使用的 JS helper |

**修改文件：**

| 文件 | 动作 |
|---|---|
| `frontend/public/translate.js` | 使用 helper 初始化卡片，监听 `app-config:changed` 刷新卡片 |
| `frontend/vite.config.ts` | Vitest 覆盖 `src/**/*.test.js` |
| `src-tauri/src/ui/config.rs` | `open_settings` 改为打开 `settings` 窗口；`save_app_config` 保存成功后广播 `app-config:changed` |
| `src-tauri/src/app/window.rs` | 新增 `show_settings_window`，设置窗口存在则显示聚焦，不存在则创建 |
| `src-tauri/src/app/popup_window.rs` | 翻译弹窗统一复用 `main` 窗口，避免启动弹窗与快捷键弹窗分裂 |
| `src-tauri/src/app/tray.rs` | 托盘「设置」改为打开 `settings` 窗口，双击托盘仍显示翻译弹窗 |
| `src-tauri/src/lib.rs` | 启动阶段不再按 `is_configured` 隐藏 `main`；应用启动即显示翻译弹窗 |
| `src-tauri/tauri.conf.json` | `main.url` 改为 `translate.html`，新增独立 `settings` 窗口配置 |
| `src-tauri/capabilities/default.json` | 窗口权限加入 `settings`，移除不再创建的 `translation-popup` |
| `README.md` | 更新当前能力和使用方式 |
| `AGENTS.md` / `CLAUDE.md` | 同步默认启动页、设置窗口、配置变更事件说明 |
| `docs/roadmap/progressive-development-plan.md` | 更新翻译弹窗 UI 能力说明 |

**任务分组：** 4 个任务：① 前端卡片同步 helper（TDD）② 配置保存广播 ③ 窗口拓扑改为 main 翻译 + settings 设置 ④ 文档同步与验收。

**跨模块高风险点：**
- `show_translation_popup`、托盘双击、快捷键翻译必须都指向同一个 `main` 翻译窗口；若仍创建 `translation-popup`，会出现两个翻译页同时接收 `translation:event`。
- `save_app_config` 必须先保存成功再广播；保存失败不广播，避免弹窗显示未落盘配置。
- 配置变更时正在流式翻译的卡片文本不能被清空；同步逻辑只调整卡片集合、顺序、标题和图标。

---

## 任务 1：前端服务卡片同步 helper

**文件：**
- 创建：`frontend/public/translate-card-sync.js`
- 创建：`frontend/public/translate-card-sync.test.js`
- 修改：`frontend/public/translate.html`
- 修改：`frontend/public/translate.js`
- 修改：`frontend/vite.config.ts`

- [ ] **步骤 1：编写失败的测试**

创建 `frontend/src/translate-card-sync.test.js`：

```js
import { describe, expect, it } from 'vitest';
import { syncServiceCards } from '../public/translate-card-sync.js';

function makeHarness(existingIds = []) {
  const order = [...existingIds];
  const resultCards = new Map(existingIds.map((id) => [
    id,
    {
      el: {
        id,
        removed: false,
        remove() {
          this.removed = true;
          const index = order.indexOf(id);
          if (index >= 0) order.splice(index, 1);
        },
      },
      name: id,
      icon: '',
      status: 'pending',
    },
  ]));
  const resultsList = {
    appendChild(el) {
      const current = order.indexOf(el.id);
      if (current >= 0) order.splice(current, 1);
      order.push(el.id);
    },
  };
  const created = [];
  const renamed = [];
  return {
    order,
    resultCards,
    resultsList,
    created,
    renamed,
    getCard(payload) {
      const id = payload.serviceInstanceId;
      let card = resultCards.get(id);
      if (!card) {
        card = { el: { id, remove() {} }, name: payload.serviceName, icon: payload.serviceType, status: 'pending' };
        resultCards.set(id, card);
        created.push(id);
      }
      return card;
    },
    updateCardMeta(card, payload) {
      card.name = payload.serviceName;
      card.icon = payload.serviceType;
      renamed.push(`${payload.serviceInstanceId}:${payload.serviceName}:${payload.serviceType}`);
    },
  };
}

describe('syncServiceCards', () => {
  it('按启用服务新增、删除并排序卡片', () => {
    const harness = makeHarness(['svc-old', 'svc-2']);

    syncServiceCards({
      services: [
        { id: 'svc-1', enabled: true, serviceType: 'deepseek', name: 'DeepSeek' },
        { id: 'svc-old', enabled: false, serviceType: 'openai', name: '旧服务' },
        { id: 'svc-2', enabled: true, serviceType: 'claude', name: 'Claude' },
      ],
      defaultSourceLang: 'auto',
      targetLang: '英文',
    }, harness);

    expect(harness.created).toEqual(['svc-1']);
    expect(harness.resultCards.has('svc-old')).toBe(false);
    expect(harness.order).toEqual(['svc-1', 'svc-2']);
  });

  it('服务名称或类型变化时更新已有卡片元信息', () => {
    const harness = makeHarness(['svc-1']);

    syncServiceCards({
      services: [
        { id: 'svc-1', enabled: true, serviceType: 'zhipu', name: '智谱 AI' },
      ],
      defaultSourceLang: 'en-US',
      targetLang: '中文',
    }, harness);

    expect(harness.renamed).toEqual(['svc-1:智谱 AI:zhipu']);
    expect(harness.resultCards.get('svc-1').name).toBe('智谱 AI');
    expect(harness.resultCards.get('svc-1').icon).toBe('zhipu');
  });
});
```

修改 `frontend/vite.config.ts` 的 `test.include`：

```ts
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.js'],
  },
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm run test -- src/translate-card-sync.test.js`

预期：FAIL，报 `Cannot find module './translate-card-sync.js'`。

- [ ] **步骤 3：实现最少 helper**

创建 `frontend/public/translate-card-sync.js`：

```js
export function servicePayload(service) {
  return {
    serviceInstanceId: service.id,
    serviceType: service.serviceType,
    serviceName: service.name,
  };
}

export function enabledServicePayloads(config) {
  return (config?.services || [])
    .filter((service) => service.enabled)
    .map(servicePayload);
}

export function syncServiceCards(config, deps) {
  const enabled = enabledServicePayloads(config);
  const enabledIds = new Set(enabled.map((service) => service.serviceInstanceId));

  deps.resultCards.forEach((card, id) => {
    if (!enabledIds.has(id)) {
      card.el.remove();
      deps.resultCards.delete(id);
    }
  });

  for (const payload of enabled) {
    const card = deps.getCard(payload);
    deps.updateCardMeta(card, payload);
    deps.resultsList.appendChild(card.el);
  }

  if (deps.langSource) {
    deps.langSource.querySelector('.lang-label').textContent =
      config.defaultSourceLang === 'auto' ? '自动检测' : config.defaultSourceLang;
  }
  if (deps.langTarget) {
    deps.langTarget.querySelector('.lang-label').textContent = config.targetLang || '中文';
  }
}
```

- [ ] **步骤 4：接入翻译弹窗**

修改 `frontend/public/translate.js` 顶部 import：

```js
import { syncServiceCards } from './translate-card-sync.js';
```

在 `getCard(payload)` 已存在卡片分支前后补一个更新函数。保留 `getCard` 的创建逻辑，在 `getCard` 后追加：

```js
function updateCardMeta(card, payload) {
  const displayName = payload.serviceName ?? '翻译';
  card.el.querySelector('.result-engine-name').textContent = displayName;
  card.el.querySelector('.result-engine-icon').innerHTML = engineIcon(
    payload.serviceType,
    payload.serviceName,
  );
}
```

把 `initCards` 改成复用同步 helper：

```js
async function refreshCardsFromConfig(config) {
  if (!config) return;
  syncServiceCards(config, {
    resultCards,
    resultsList,
    getCard,
    updateCardMeta,
    langSource,
    langTarget,
  });
  adjustHeight();
}

async function initCards() {
  if (!invoke) return;
  try {
    await refreshCardsFromConfig(await invoke('get_app_config'));
  } catch {
    return;
  }
}
```

在现有 `listen('translation:event', ...)` 旁追加配置变更监听：

```js
if (listen) {
  listen('app-config:changed', (event) => {
    refreshCardsFromConfig(event.payload);
  });
}
```

- [ ] **步骤 5：运行前端测试和语法检查**

运行：

```bash
npm run test -- src/translate-card-sync.test.js
node --check frontend/public/translate.js
node --check frontend/public/translate-card-sync.js
```

预期：Vitest PASS；两个 `node --check` 均无输出。

- [ ] **步骤 6：Commit**

```bash
git add frontend/public/translate-card-sync.js frontend/src/translate-card-sync.test.js frontend/public/translate.js frontend/vite.config.ts
git commit -m "feat(popup): 同步启用服务卡片"
```

---

## 任务 2：配置保存成功后广播 app-config:changed

**文件：**
- 修改：`src-tauri/src/ui/config.rs`

- [ ] **步骤 1：编写最小后端改动**

修改 import，加入 `tauri::Emitter`：

```rust
use tauri::Emitter;
```

把 `save_app_config` 末尾保存逻辑改为保存后广播：

```rust
    let saved_config = state
        .config_store
        .save(config)
        .map_err(|error| ShortcutBindingError::global(format!("无法保存配置: {error}")))?;

    app.emit("app-config:changed", &saved_config)
        .map_err(|error| ShortcutBindingError::global(format!("无法广播配置变更: {error}")))?;

    Ok(saved_config)
```

保持现有顺序不变：读取旧配置 → `normalized()` → `replace_global_shortcuts` → 保存 → 广播。

- [ ] **步骤 2：运行后端构建验证**

运行：`cd src-tauri && cargo build`

预期：编译通过。若 `ShortcutBindingError` 类型不接受该错误构造，复用当前文件第 30/38 行已有的 `ShortcutBindingError::global(format!(...))` 写法修正。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ui/config.rs
git commit -m "feat(config): 保存配置后广播变更事件"
```

---

## 任务 3：启动窗口改为翻译弹窗，设置页独立窗口

**文件：**
- 修改：`src-tauri/tauri.conf.json`
- 修改：`src-tauri/capabilities/default.json`
- 修改：`src-tauri/src/app/window.rs`
- 修改：`src-tauri/src/app/popup_window.rs`
- 修改：`src-tauri/src/ui/config.rs`
- 修改：`src-tauri/src/app/tray.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：修改 Tauri 窗口配置**

把 `src-tauri/tauri.conf.json` 的 `app.windows` 改为两个窗口：

```json
    "windows": [
      {
        "label": "main",
        "url": "translate.html",
        "title": "Shizi - 翻译助手",
        "width": 420,
        "height": 480,
        "resizable": false,
        "decorations": false,
        "transparent": true,
        "center": true
      },
      {
        "label": "settings",
        "url": "settings.html",
        "title": "Shizi - 设置",
        "width": 560,
        "height": 640,
        "minWidth": 480,
        "minHeight": 480,
        "resizable": true,
        "center": true,
        "visible": false
      }
    ]
```

修改 `src-tauri/capabilities/default.json`：

```json
  "windows": ["main", "settings", "screenshot-overlay"],
```

- [ ] **步骤 2：实现独立设置窗口 helper**

修改 `src-tauri/src/app/window.rs`：

```rust
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn show_settings_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = match app.get_webview_window("settings") {
        Some(window) => window,
        None => WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
            .title("Shizi - 设置")
            .inner_size(560.0, 640.0)
            .min_inner_size(480.0, 480.0)
            .resizable(true)
            .center()
            .build()
            .map_err(|error| format!("创建设置窗口失败: {error}"))?,
    };
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}
```

保留 `setup_close_to_hide` 现有逻辑，继续只处理 `main` 翻译窗口。

- [ ] **步骤 3：open_settings 改为打开 settings 窗口**

修改 `src-tauri/src/ui/config.rs` import：

```rust
        window::show_settings_window,
```

把 `open_settings` 改为：

```rust
#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}
```

- [ ] **步骤 4：翻译弹窗统一复用 main 窗口**

修改 `src-tauri/src/app/popup_window.rs`：

```rust
pub const POPUP_LABEL: &str = "main";
```

把 `ensure_popup_window` 改成不再创建额外 `translation-popup`：

```rust
pub fn ensure_popup_window(_app: &tauri::AppHandle, _config: &AppConfig) -> Result<(), String> {
    Ok(())
}
```

把 `show_popup` 的窗口获取逻辑改为始终复用 `main`：

```rust
pub fn show_popup(app: &tauri::AppHandle, _config: &AppConfig) -> Result<(), String> {
    let window = app
        .get_webview_window(POPUP_LABEL)
        .ok_or_else(|| "翻译弹窗未创建".to_string())?;
```

删除 `show_popup` 中 `config.popup_precreate` 分支和 `build_popup(app)?` 分支，保留后续光标定位、`show()`、`set_focus()` 逻辑。删除不再使用的 `build_popup` 函数和 `WebviewUrl` / `WebviewWindowBuilder` / `webview::Color` import。

- [ ] **步骤 5：托盘设置入口改为 settings 窗口**

修改 `src-tauri/src/app/tray.rs` import：

```rust
use crate::app::window::{show_settings_window, show_window};
```

把菜单事件 `"settings"` 分支改为：

```rust
            "settings" => {
                let _ = show_settings_window(app);
            }
```

双击托盘保留 `show_window(tray.app_handle())`，现在它显示 `main` 翻译窗口。

- [ ] **步骤 6：启动阶段显示翻译弹窗**

修改 `src-tauri/src/lib.rs` setup 中第 68-83 行附近逻辑，删除 `is_configured()` 决定隐藏/显示 `main` 的分支，改为：

```rust
            // 启动即显示 main 翻译弹窗；设置页由 open_settings 独立打开。
            let _ = ensure_popup_window(app.handle(), &config);
            let _ = ensure_overlay(app.handle());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
```

- [ ] **步骤 7：运行构建验证**

运行：

```bash
cd src-tauri && cargo build
npm run build
```

预期：Rust 编译通过；前端构建通过且 `frontend/dist/translate.html` 来自 `frontend/public/translate.html`。

- [ ] **步骤 8：Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/capabilities/default.json src-tauri/src/app/window.rs src-tauri/src/app/popup_window.rs src-tauri/src/ui/config.rs src-tauri/src/app/tray.rs src-tauri/src/lib.rs
git commit -m "feat(window): 启动显示翻译弹窗并独立设置窗口"
```

---

## 任务 4：文档同步与最终验收

**文件：**
- 修改：`README.md`
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`docs/superpowers/plans/2026-07-06-startup-translation-popup-live-services.md`

- [ ] **步骤 1：同步 README**

更新 `README.md` 中当前能力与使用方式，至少包含以下事实：

```markdown
- 启动即显示翻译弹窗；设置页为独立窗口，可从翻译弹窗设置按钮或托盘「设置」打开。
- 设置页保存服务启用/关闭后，会通过 `app-config:changed` 通知已打开的翻译弹窗实时同步结果卡片，无需重启。
```

把“手动翻译”第 1 步改为：

```markdown
1. 启动应用，默认显示翻译弹窗。
```

- [ ] **步骤 2：同步 AGENTS.md 与 CLAUDE.md**

两个文件必须保持一致。更新架构关键点中的相关描述：

```markdown
- **启动窗口与设置窗口**：`main` 窗口加载 `translate.html`，应用启动后默认显示翻译弹窗；设置页由独立 `settings` 窗口加载 `settings.html`，通过弹窗设置按钮、托盘「设置」或 `open_settings` command 打开。
- **配置变更事件**：`save_app_config` 保存成功后广播 `app-config:changed`，翻译弹窗监听该事件并按启用服务列表新增、删除、排序和更新卡片，无需重启。
```

删除或改写仍声称“主窗口设置页”的旧描述。

- [ ] **步骤 3：同步 roadmap**

在 `docs/roadmap/progressive-development-plan.md` 中把翻译弹窗 UI 能力更新为：

```markdown
- 启动后默认显示翻译弹窗；设置页独立窗口打开。
- 服务启用状态保存后，翻译弹窗结果卡片实时同步。
```

- [ ] **步骤 4：回填计划复选框**

在本计划中把已完成任务的复选框从 `- [ ]` 改为 `- [x]`。只勾选已经实际执行并验证通过的步骤。

- [ ] **步骤 5：最终验证**

运行：

```bash
npm run test
npm run typecheck
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：全部通过。

手动验证：

```bash
npm run tauri dev
```

逐项确认：

1. 应用启动后显示翻译弹窗，不显示设置页。
2. 翻译弹窗设置按钮打开 `settings` 设置窗口。
3. 托盘「设置」打开同一个设置窗口。
4. 关闭/重新打开设置窗口后，设置按钮仍能再次打开。
5. 设置页启用/关闭服务并保存后，翻译弹窗卡片立即新增、删除并按服务顺序重排。
6. 翻译流式输出过程中保存配置，不清空正在输出的卡片文本。

- [ ] **步骤 6：Commit**

```bash
git add README.md AGENTS.md CLAUDE.md docs/roadmap/progressive-development-plan.md docs/superpowers/plans/2026-07-06-startup-translation-popup-live-services.md
git commit -m "docs: 同步启动翻译弹窗与配置实时刷新"
```

---

## 自检结果

**1. 规格覆盖度：**
- 启动后默认展示翻译弹窗：任务 3 修改 `tauri.conf.json` 与 `lib.rs`。
- 设置按钮打开设置页：任务 3 修改 `open_settings`、`show_settings_window`、托盘设置入口。
- 保存服务启用/关闭后实时同步卡片：任务 1 前端监听和同步 helper，任务 2 后端广播事件。
- 不迁移翻译页到 Vue、不重做样式、不改翻译链路：计划只新增静态 JS helper，保留 `translation:event` 渲染和现有 CSS。
- 测试：任务 1 覆盖卡片新增、删除、排序和元信息更新；任务 3/4 覆盖构建和手动验证。
- 文档同步：任务 4 覆盖 README、AGENTS、CLAUDE、roadmap、计划复选框。

**2. 占位符扫描：**
- 本计划已清除空泛占位表达。
- 每个代码改动步骤都给出具体文件、代码片段、命令和预期结果。

**3. 类型一致性：**
- 前端事件名统一为 `app-config:changed`，后端 `app.emit` 与前端 `listen` 一致。
- 服务字段使用后端序列化后的 camelCase：`serviceType`、`defaultSourceLang`、`targetLang`。
- 翻译窗口 label 统一为 `main`；设置窗口 label 统一为 `settings`。
- `show_settings_window(&tauri::AppHandle) -> Result<(), String>` 在 `window.rs` 定义，在 `ui/config.rs` 与 `tray.rs` 使用。
