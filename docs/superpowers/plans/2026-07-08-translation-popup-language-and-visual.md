# 翻译弹窗语言联动与卡片视觉优化 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法跟踪进度。任务卡详件见 `docs/agent/2026-07-08-translation-popup-language-and-visual-backend.md` 与 `docs/agent/2026-07-08-translation-popup-language-and-visual-frontend.md`。

**目标：** 4 个翻译弹窗优化一并落地——①弹窗源/目标语言与会话语言状态联动（运行时临时修改、重启重置）；②设置页源语言补回「自动检测」+ 语言代码统一为 `zh-CN`；③main 窗口任务栏不显示图标；④翻译进行中卡片头部闪动蓝点（保留流式光标）。不新增后端翻译业务逻辑，保持 translate.html 纯静态。

**架构：** 后端 AppState 新增会话语言字段（内存态，启动从 config 初始化，不持久化），两个 Tauri command 暴露 get/set；所有翻译入口（手动 / 划词 / OCR / 重试）经 `start_translation_from_input` 统一读会话语言构造批次。前端 translate.js 维护 LANGUAGES 代码↔名称映射（适配层），下拉改语言调 set command，卡片头部蓝点由 translation:event 驱动。后端契约只传语言**代码**（如 `auto`/`zh-CN`），显示名由前端映射——核心原则 8（后端 UI 无关）。

**技术栈：** Rust（`tauri::command` + 现有 `Arc<Mutex>` 状态模式）、原生 ES module（`frontend/public/translate.js`）、Vue 3 + vitest（设置页）。

---

## 与 spec 的实现澄清

spec（`docs/superpowers/specs/2026-07-08-translation-popup-language-and-visual-design.md`）是功能事实来源，以下为实现层澄清，不改变功能要求：

1. **AppState 会话语言字段用 `Arc<Mutex<String>>`，非 spec 所写的 `RwLock`。** 现有 `AppState`（`src-tauri/src/app/state.rs`）所有运行时字段统一用 `Arc<Mutex<...>>`（pending_source_text、translation_busy、capture_in_progress 等）。为保持风格一致，会话语言字段沿用 `Arc<Mutex<String>>`。RwLock 在此场景无收益（读写比不倾斜、不持锁调复杂代码），Mutex 足够且与现有代码一致。spec §4.1 的 `RwLock` 仅为示意，实现以现有模式为准。

2. **`SessionLanguages` 只需 `Serialize`。** `get_session_languages` 返回它（Serialize）；`set_session_languages` 接收两个 `String` 参数（非结构体），Tauri v2 自动把前端 `{ sourceLang, targetLang }` 映射到 snake_case 参数。故 `SessionLanguages` 不需要 `Deserialize`（YAGNI）。

3. **`deserializes_with_defaults` 测试需同步改输入与断言。** spec §5.2 指向 types.rs:408 `assert_eq!(config.target_lang, "中文")`，该断言位于 `deserializes_with_defaults` 测试，其输入 JSON 显式提供 `"targetLang": "中文"`（非空，`normalize_string` 保留原值）。仅改 `DEFAULT_TARGET_LANG` 不会触发该断言变化。故实现需同时把该测试的输入 JSON 与断言改为 `"zh-CN"`，并新增 `from_env_default_target_lang_is_zh_cn` 测试显式断言默认值。

4. **`.lang-side` CSS 当前不支持点击（`cursor: default` 无 `:hover`）。** spec §4.2 称「CSS 已支持」不准确。前端任务需补 `.lang-side { cursor: pointer }` + `:hover { background: var(--bg-soft) }` + `.lang-chevron` 样式，并在 translate.html 加 chevron svg。

5. **`start_translation_from_input` 是所有翻译入口的唯一汇聚点。** 已确认：手动 `start_translation_from_text`（web_popup.rs:43）、划词 `shortcuts.rs:308`、OCR `overlay.rs:160`、重试 `web_popup.rs:257` 全部调用 `start_translation_from_input`。故只改这一处即可让所有入口用上会话语言，无需改 `build_batch_requests` 签名。

---

## 后端契约段（UI 无关）

> 本段为架构师独占写权限的契约定义（agent-team.md §5.1）。前后端 subagent 只读消费，引用契约 ID。字段一律领域语言，禁止 `display_*`/`formatted_*`/已本地化文案。语言字段是**代码**（如 `auto`/`zh-CN`/`en-US`），不是显示名。

### C-1：`SessionLanguages` IPC DTO

```rust
/// 会话语言（运行时内存态）。字段为语言代码，非显示名。
/// 序列化为 `{ "sourceLang": "auto", "targetLang": "zh-CN" }`。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLanguages {
    pub source_lang: String,
    pub target_lang: String,
}
```

- `source_lang`：源语言代码，`"auto"` 表示自动检测。
- `target_lang`：目标语言代码，永不为 `"auto"`（前端下拉过滤 + 交换跳过双重保证）。
- 未知代码（如旧 config 残留的 `"中文"`）原样透传，前端 `LANG_LABEL` 回退显示原值。

### C-2：`get_session_languages` command

```rust
#[tauri::command]
pub async fn get_session_languages(
    state: tauri::State<'_, AppState>,
) -> Result<SessionLanguages, String>;
```

- 返回当前 AppState 会话语言。
- 前端 `invoke('get_session_languages')` -> `Promise<{ sourceLang: string, targetLang: string }>`。
- 失败返回 `Err(String)`（RwLock/Mutex 毒化时，实际回退 `("auto", "zh-CN")` 不返回错误——见 C-6 内部方法）。

### C-3：`set_session_languages` command

```rust
#[tauri::command]
pub async fn set_session_languages(
    source_lang: String,
    target_lang: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String>;
```

- Tauri v2 自动把前端 `invoke('set_session_languages', { sourceLang, targetLang })` 映射到 snake_case 参数。
- 写入 AppState 内存，**不持久化**到 config.json。
- **不校验语言代码**（spec 未要求；前端下拉控制可选值，后端只存原值）。见 open-questions OQ-3。
- 失败返回 `Err(String)`（锁毒化）。

### C-4：`AppConfig.default_source_lang` / `target_lang` 字段语义

- `default_source_lang: String`：默认源语言代码，默认值 `"auto"`。启动时用于初始化会话源语言。
- `target_lang: String`：默认目标语言代码，默认值由 `"中文"` 改为 `"zh-CN"`（统一为代码）。启动时用于初始化会话目标语言。
- 这两个字段是**持久化默认**（config.json），运行时翻译**不直接读它们**——翻译读会话语言（C-1）。仅在启动 `AppState::new` 时作为会话语言初值。
- 设置页 `save_app_config` 保存后广播 `app-config:changed`，弹窗监听后**只刷新服务卡片，不重置会话语言**（会话语言独立于 config，重启才重置）。
- 前端 `AppConfig` 类型（`frontend/src/types/config.ts`）的 `targetLang`/`defaultSourceLang` 字段与之同源同义（camelCase 序列化）。

### C-5：`translation:event` 事件 schema（不变）

已有事件，**schema 不变**。`Started`/`Delta`/`Finished`/`Failed`/`Cancelled` 变体字段保持现状。前端据 `started`/`delta` 显示卡片头部蓝点，`finished`/`failed`/`cancelled` 隐藏蓝点（见前端分册 FE-3）。本计划不改事件结构。

### C-6：AppState 会话语言内部方法（后端内部，非契约）

> 此为后端实现细节，非跨端契约。前端不感知。列出仅为后端任务卡引用。

```rust
impl AppState {
    /// 读会话源/目标语言。锁毒化时回退 ("auto", "zh-CN")，不返回 Err。
    pub fn session_languages(&self) -> (String, String);
    /// 写会话源/目标语言。锁毒化返回 Err。
    pub fn set_session_languages(&self, source: String, target: String) -> Result<(), String>;
}
```

---

## 共享数据结构

| 数据 | 归属 | 说明 |
|---|---|---|
| `SessionLanguages` (Rust struct) | 后端 web_popup.rs | C-1，Serialize，camelCase |
| `{ sourceLang, targetLang }` (JS) | 前端 | C-1 的 JS 视图，invoke 参数/返回 |
| `LANGUAGES` 代码↔名称映射 | 前端 translate.js + settings/tokens.ts | 两处同源（spec §4.2），新增语言两处同步。后端不持有此映射 |
| 会话语言状态（`session_source_lang`/`session_target_lang`） | 后端 AppState | 内存态，Arc<Mutex<String>>，启动从 config 初始化 |
| `DEFAULT_TARGET_LANG = "zh-CN"` | 后端 types.rs | 持久化默认，C-4 |
| `defaultTargetLang = 'zh-CN'` | 前端 settings.ts | 前端默认状态（pre-sync），与后端默认保持一致 |

---

## 里程碑与任务 DAG

### 里程碑

- **M1 后端契约就绪**：BE-1（AppState 会话语言）+ BE-2（DEFAULT_TARGET_LANG）+ BE-3（commands + 翻译入口改造）+ BE-4（注册 command）+ BE-5（skipTaskbar）。`cargo test` + `cargo build` 通过。
- **M2 前端适配就绪**：FE-1（settings 默认值 + 回读）+ FE-2（TranslatePanel auto 选项）+ FE-3（弹窗语言下拉 + 卡片蓝点）。`npm run typecheck` + `npm run test` + `npm run build` 通过。
- **M3 集成验证**：`npm run tauri dev` 手动验证 spec §10 清单（10 项）。

### 任务 DAG

```
后端：
BE-1 (state.rs) ──┬──> BE-3 (web_popup.rs) ──> BE-4 (lib.rs)
BE-2 (types.rs) ──┘                                  
BE-5 (tauri.conf.json)  [独立，无依赖]

前端（均无硬依赖后端，按契约编码；手动验证需 BE-3/BE-4 就绪）：
FE-1 (settings.ts/test)  [独立]
FE-2 (TranslatePanel.vue) [独立]
FE-3 (translate.js/html/css/card-sync) [独立]
```

| 任务 | owner | depends_on | can_parallel_with | model_tier |
|---|---|---|---|---|
| BE-1 | backend | — | BE-2, BE-5, FE-1, FE-2, FE-3 | weak |
| BE-2 | backend | — | BE-1, BE-3*, BE-5, FE-1, FE-2, FE-3 | weak |
| BE-3 | backend | BE-1 | BE-2, BE-5, FE-1, FE-2, FE-3 | weak |
| BE-4 | backend | BE-3 | BE-5, FE-1, FE-2, FE-3 | weak |
| BE-5 | backend | — | BE-1, BE-2, BE-3, BE-4, FE-1, FE-2, FE-3 | weak |
| FE-1 | frontend | — | BE-*, FE-2, FE-3 | weak |
| FE-2 | frontend | — | BE-*, FE-1, FE-3 | weak |
| FE-3 | frontend | — | BE-*, FE-1, FE-2 | weak |

\* BE-2 与 BE-3 文件无交集（types.rs vs web_popup.rs），但 BE-3 依赖 BE-1；BE-2 可与 BE-3 并行（BE-1 完成后）。

**串行链**：BE-1 -> BE-3 -> BE-4（后端唯一串行链，因 command 定义在 web_popup.rs、注册在 lib.rs、AppState 方法在 state.rs 三者跨文件依赖）。
**可并行**：BE-2 / BE-5 / FE-1 / FE-2 / FE-3 全程可并行于上述串行链（文件无交集）。

**文件锁检查**（PM dispatch 前置，§5.2）：
- BE-1 独占 `src-tauri/src/app/state.rs`
- BE-2 独占 `src-tauri/src/core/config/types.rs`
- BE-3 独占 `src-tauri/src/ui/web_popup.rs`
- BE-4 独占 `src-tauri/src/lib.rs`
- BE-5 独占 `src-tauri/tauri.conf.json`
- FE-1 独占 `frontend/src/settings/stores/settings.ts` + `settings.test.ts`
- FE-2 独占 `frontend/src/settings/panels/TranslatePanel.vue`
- FE-3 独占 `frontend/public/translate.js` + `translate.html` + `translate.css` + `translate-card-sync.js`

无交集，全部可按 DAG 并行调度。

---

## 验收标准

### 后端

- `cd src-tauri && cargo test` 全绿，含新增：
  - `session_languages_init_from_config`（BE-1）
  - `set_session_languages_updates_state`（BE-1）
  - `set_session_languages_persists_until_reset`（BE-1，set 后改 config 不影响会话语言）
  - `from_env_default_target_lang_is_zh_cn`（BE-2）
  - `deserializes_with_defaults` 输入与断言改为 `"zh-CN"`（BE-2）
- `cd src-tauri && cargo build` 通过（tauri.conf.json 的 `skipTaskbar` 编译期校验）。
- `start_translation_from_input` 内 `build_batch_requests` 调用从 `config.target_lang`/`config.default_source_lang` 改为 `state.session_languages()`。

### 前端

- `npm run typecheck` 通过。
- `npm run test` 全绿，含新增 `defaultTargetLang` 默认 `'zh-CN'` 断言与 `syncFromBackend` 回读 `targetLang` 断言（FE-1）。
- `npm run build` 通过（translate.js/html/css 为静态资源，不参与 Vite 构建但 build 不报错）。

### 集成手动验证（spec §10 清单，`npm run tauri dev`）

1. 启动软件，弹窗源=自动检测、目标=简体中文（与设置页默认一致）。
2. 弹窗下拉切目标为 English，翻译一段中文 -> 译成英文。
3. 划词翻译（Alt+D）-> 用弹窗临时设的 English 目标（验证划词也用会话语言）。
4. 关闭弹窗（hide）再唤起 -> 临时语言保留。
5. 退出软件重启 -> 语言重置为设置页默认。
6. 设置页源语言下拉可选「自动检测」；目标语言下拉无「自动检测」。
7. 设置页改默认目标语言 -> 弹窗本次运行不跟变；重启后跟变。
8. 任务栏无翻译弹窗图标；settings 窗口有任务栏图标。
9. 翻译长文本：卡片头部蓝点持续闪动，卡片内光标闪烁；翻译完成后蓝点消失、光标消失。
10. 交换语言按钮：非 auto 时交换；含 auto 时 toast 提示。

---

## open-questions

> 以下为 spec 矛盾/遗漏/灰色地带，实现按「实现澄清」与任务卡说明处理，**不自行改 spec**。PM 决策是否回填 spec。

- **OQ-1（spec 不准确，非阻断）**：spec §4.2 称 `.lang-side` 「CSS 已支持」可点击，但 `translate.css` 现有 `.lang-side` 为 `cursor: default` 且无 `:hover`。前端任务 FE-3 补齐 `cursor: pointer` + `:hover` + `.lang-chevron` 样式（适配层职责，非 spec 变更）。

- **OQ-2（spec 未提及，范围确认）**：`translate.html` 原文卡 `.source-meta` 内有 `<span class="lang-badge">自动检测</span>`（与 `.lang-toolbar` 的 `langSource` 是不同元素）。spec 只要求 `langSource`/`langTarget` 联动会话语言，未提及 `.lang-badge`。**本次不动 `.lang-badge`**（保持「自动检测」静态文案），避免范围蔓延。若 PM 要求 `.lang-badge` 跟随会话源语言，另起任务。

- **OQ-3（spec 未要求，灰色地带）**：`set_session_languages` 后端是否校验语言代码？spec 未要求。按核心原则 8，后端本应健壮，但前端下拉已限制可选值，后端校验收益低。**本次不校验**，后端原样存原值。若后续需校验，后端加代码白名单（UI 无关，纯领域校验）。

- **OQ-4（spec 行引用偏差，非阻断）**：spec §5.2 称「测试 `normalized_fills_ui_runtime_defaults`（types.rs:408）」，实际 types.rs:408 属 `deserializes_with_defaults` 测试，且其输入 JSON 显式提供 `"targetLang": "中文"`。BE-2 任务卡按正确语义处理：改 `DEFAULT_TARGET_LANG` + 同步改 `deserializes_with_defaults` 输入与断言为 `"zh-CN"` + 新增 `from_env_default_target_lang_is_zh_cn` 显式断言默认值。

- **OQ-5（spec 写 RwLock，实际 Mutex，非阻断）**：spec §4.1 示意 `RwLock<String>`，现有 AppState 统一用 `Arc<Mutex<String>>`。BE-1 沿用 `Mutex`（风格一致，无并发收益差异）。实现澄清第 1 条已说明。

---

## 自检

### 1. 规格覆盖度

逐条对照 spec 4 个需求：

- ✅ 需求 1（语言联动）：后端 AppState 会话语言（BE-1）+ 两个 command（BE-3）+ 翻译入口读会话语言（BE-3）+ 前端下拉/交换/init 读会话语言（FE-3）。生命周期（启动初始化、运行时临时、重启重置）由 C-1/C-4 与 BE-1 保证。
- ✅ 需求 2（源语言 auto + 代码统一）：TranslatePanel 源补 auto + 目标过滤 auto（FE-2）+ DEFAULT_TARGET_LANG 改 zh-CN（BE-2）+ settings.ts defaultTargetLang 改 zh-CN + syncFromBackend 回读 targetLang（FE-1）。
- ✅ 需求 3（任务栏图标）：tauri.conf.json main 加 skipTaskbar（BE-5）。
- ✅ 需求 4（卡片视觉）：卡片头部蓝点 setHeaderDot + CSS pulse-dot（FE-3），流式光标保持不变（已实现）。

spec「不做」：不改 prompt 生成、不改 build_batch_requests 签名、不加后端代码->名称映射、不碰 overlay/settings 其他模块、不改 settings 窗口任务栏、不迁 Vue——均遵守。

### 2. 契约 UI 无关性

- C-1 `SessionLanguages` 字段 `source_lang`/`target_lang` 为代码，无 `display_*`/`formatted_*`。✅
- C-2/C-3 command 参数/返回为代码字符串，无 UI 文案。✅
- C-4 `AppConfig` 字段为代码，无本地化文案。✅
- C-5 事件 schema 不变，已 UI 无关。✅
- 显示名映射（LANGUAGES）归前端（FE-3），后端不持有。✅
- `skipTaskbar` 是窗口配置（tauri.conf.json），非权限（capabilities/），不混淆（反模式 §11.24）。✅

### 3. 任务粒度与文件锁

- 8 个任务，owner 单一（backend/frontend），无 `both`。✅
- 每任务 `files_to_write` 显式且无交集（见 DAG 文件锁检查）。✅
- 唯一串行链 BE-1->BE-3->BE-4（跨文件依赖）；其余可并行。✅

### 4. 模型决策门

- 全部实现任务 weak（sonnet）：BE-1 的 Mutex 用法简单（读写两个 String，不持锁调复杂代码，无跨模块状态机），不触发升级；BE-2/3/4/5 常规；FE-1/2/3 常规前端适配。✅
- 架构师（本角色）= strong（opus），把关层。✅
- 无任务因「主会话用强模型」而继承强模型。✅
