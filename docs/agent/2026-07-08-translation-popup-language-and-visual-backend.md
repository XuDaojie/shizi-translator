# 后端任务清单 · 翻译弹窗语言联动与卡片视觉优化

> 主 plan：`docs/superpowers/plans/2026-07-08-translation-popup-language-and-visual.md`（契约段在此文件的「后端契约段」）。
> 本文件为后端任务卡切片，每任务引用契约 ID，**明确禁止 UI 语义**（核心原则 8）。PM 按此 dispatch，subagent 只读本任务卡 + 主 plan 契约段 + `files_to_read`。

## 任务总览

| task_id | 标题 | owner | depends_on | model_tier |
|---|---|---|---|---|
| BE-1 | AppState 会话语言字段 + 方法 + 启动初始化（TDD） | backend | - | weak |
| BE-2 | DEFAULT_TARGET_LANG 改 `zh-CN` + 测试（TDD） | backend | - | weak |
| BE-3 | SessionLanguages DTO + get/set command + 翻译入口读会话语言 | backend | BE-1 | weak |
| BE-4 | lib.rs 注册两个 command | backend | BE-3 | weak |
| BE-5 | tauri.conf.json main 加 skipTaskbar | backend | - | weak |

---

## BE-1：AppState 会话语言字段 + 方法 + 启动初始化（TDD）

- **task_id**：BE-1
- **owner**：backend
- **files_to_write**：
  - `src-tauri/src/app/state.rs`
- **files_to_read**：
  - `src-tauri/src/app/state.rs`（现有 AppState 结构与 `new` 方法）
  - `src-tauri/src/core/config/types.rs`（AppConfig 的 `default_source_lang`/`target_lang` 字段）
  - `src-tauri/src/core/config/store.rs`（ConfigStore::get 接口）
  - 主 plan 契约段 C-4 / C-6
- **contract_refs**：C-4（AppConfig 字段语义）、C-6（内部方法签名）
- **depends_on**：-
- **can_parallel_with**：BE-2、BE-5、FE-1、FE-2、FE-3
- **model_tier**：weak
- **tier_rationale**：默认弱模型。任务涉及 `Arc<Mutex<String>>` 并发原语，但用法极简（读写两个 String，不持锁调用复杂代码，无跨模块状态机），不触发强模型升级条件（§5.6）。Mutex 沿用现有 AppState 风格（实现澄清第 1 条）。
- **boundary_rationale**：会话语言是运行时业务状态（翻译入口消费），非 UI 渲染数据。存语言代码（非显示名），UI 无关。归属后端无疑义。

### 实现要点

**字段**（在 `AppState` struct 末尾、`shortcut_conflicts` 之后加）：

```rust
    // 会话语言（运行时内存态）：启动从 config 初始化，前端 set_session_languages
    // 写入，所有翻译入口经 start_translation_from_input 读取。不持久化，重启重置。
    // 存语言代码（如 "auto"/"zh-CN"），非显示名。
    session_source_lang: Arc<Mutex<String>>,
    session_target_lang: Arc<Mutex<String>>,
```

**`AppState::new`**（在 `shortcut_conflicts: ...` 之后初始化）：

```rust
            shortcut_conflicts: Arc::new(Mutex::new(Vec::new())),
            session_source_lang: {
                let lang = config_store
                    .get()
                    .map(|c| c.default_source_lang)
                    .unwrap_or_else(|_| "auto".to_string());
                Arc::new(Mutex::new(lang))
            },
            session_target_lang: {
                let lang = config_store
                    .get()
                    .map(|c| c.target_lang)
                    .unwrap_or_else(|_| "zh-CN".to_string());
                Arc::new(Mutex::new(lang))
            },
```

> 注：`config_store.get()` 失败回退 `"auto"`/`"zh-CN"`（与 spec §4.1 一致）。`new(config_store)` 接收 `ConfigStore` by value，但 `ConfigStore` 内部是 `Arc<RwLock<AppConfig>>`（见 state.rs:278 测试 `ConfigStore::from_parts_for_test`），`get()` 借用 `&self`/`&self.config_store`。实现时确认 `config_store.get()` 在 `new` 内可调（`ConfigStore::get(&self)`）。

**方法**（在 `shortcut_conflicts` 方法之后加）：

```rust
    /// 读会话源/目标语言。锁毒化回退 ("auto", "zh-CN")，不返回 Err
    ///（翻译入口不应因状态读失败而阻断）。
    pub fn session_languages(&self) -> (String, String) {
        let source = self
            .session_source_lang
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "auto".to_string());
        let target = self
            .session_target_lang
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "zh-CN".to_string());
        (source, target)
    }

    /// 写会话源/目标语言。锁毒化返回 Err。不持久化。
    pub fn set_session_languages(
        &self,
        source: String,
        target: String,
    ) -> Result<(), String> {
        let mut s = self
            .session_source_lang
            .lock()
            .map_err(|_| "会话源语言锁已损坏".to_string())?;
        let mut t = self
            .session_target_lang
            .lock()
            .map_err(|_| "会话目标语言锁已损坏".to_string())?;
        *s = source;
        *t = target;
        Ok(())
    }
```

### TDD 步骤

- [ ] **步骤 1：编写失败测试**（在 `state.rs` 的 `#[cfg(test)] mod tests` 末尾加）

```rust
    #[test]
    fn session_languages_init_from_config() {
        let mut config = AppConfig::from_env();
        config.default_source_lang = "en-US".to_string();
        config.target_lang = "ja-JP".to_string();
        let state = AppState::new(ConfigStore::from_parts_for_test(
            PathBuf::from("unused-config.json"),
            Arc::new(RwLock::new(config)),
        ));
        let (source, target) = state.session_languages();
        assert_eq!(source, "en-US");
        assert_eq!(target, "ja-JP");
    }

    #[test]
    fn set_session_languages_updates_state() {
        let state = app_state();
        state
            .set_session_languages("en-US".to_string(), "zh-CN".to_string())
            .expect("set 应成功");
        let (source, target) = state.session_languages();
        assert_eq!(source, "en-US");
        assert_eq!(target, "zh-CN");
    }

    #[test]
    fn set_session_languages_persists_until_reset() {
        // set 后修改 config_store 的 config，会话语言不应跟随变化
        let mut config = AppConfig::from_env();
        config.target_lang = "zh-CN".to_string();
        let store = Arc::new(RwLock::new(config));
        let state = AppState::new(ConfigStore::from_parts_for_test(
            PathBuf::from("unused-config.json"),
            store.clone(),
        ));
        state
            .set_session_languages("auto".to_string(), "en-US".to_string())
            .expect("set 应成功");
        // 改 config 的 target_lang
        store.write().unwrap().target_lang = "ja-JP".to_string();
        // 会话语言仍是 set 的值
        let (source, target) = state.session_languages();
        assert_eq!(source, "auto");
        assert_eq!(target, "en-US");
    }
```

> 注：`app_state()` 辅助已在 state.rs:276 存在。`from_parts_for_test` 已存在（state.rs:278）。`Arc<RwLock<AppConfig>>` 的 `RwLock` 是 `std::sync::RwLock`（确认 imports 已有）。

- [ ] **步骤 2：运行测试验证失败**
  - `cd src-tauri && cargo test --lib app::state::tests`
  - 预期：FAIL，`no field session_source_lang on type AppState`（编译错误）。

- [ ] **步骤 3：实现字段 + new 初始化 + 方法**（按「实现要点」插入）

- [ ] **步骤 4：运行测试验证通过**
  - `cd src-tauri && cargo test --lib app::state::tests`
  - 预期：PASS（含新增 3 个测试 + 原有全过）。

- [ ] **步骤 5：Commit**（PM 串行 commit，subagent 不自行 commit）

### acceptance

- `cd src-tauri && cargo test --lib app::state::tests` 全绿。
- `cd src-tauri && cargo build` 通过。
- `AppState::new` 启动即从 config 初始化会话语言（不依赖额外调用）。
- 会话语言读写不持锁调用复杂代码（无死锁风险）。

---

## BE-2：DEFAULT_TARGET_LANG 改 `zh-CN` + 测试（TDD）

- **task_id**：BE-2
- **owner**：backend
- **files_to_write**：
  - `src-tauri/src/core/config/types.rs`
- **files_to_read**：
  - `src-tauri/src/core/config/types.rs`（DEFAULT_TARGET_LANG、from_env、normalized、deserializes_with_defaults 测试）
  - 主 plan 契约段 C-4、open-questions OQ-4
- **contract_refs**：C-4（target_lang 默认 `zh-CN`）
- **depends_on**：-
- **can_parallel_with**：BE-1、BE-3（BE-1 完成后）、BE-5、FE-1、FE-2、FE-3
- **model_tier**：weak
- **tier_rationale**：默认弱模型。常量改值 + 测试断言更新，无复杂逻辑。
- **boundary_rationale**：持久化默认值属后端 config 层。前端 settings.ts 各自维护同源默认（FE-1），两处手动同步（既有模式，如 `default_source_lang: "auto"` 已如此）。

### 实现要点

- [ ] **步骤 1：改 DEFAULT_TARGET_LANG**

`types.rs:6`：

```rust
const DEFAULT_TARGET_LANG: &str = "zh-CN";
```

- [ ] **步骤 2：改 `deserializes_with_defaults` 测试**（types.rs:401-408）

输入 JSON 的 `"targetLang": "中文"` 改为 `"zh-CN"`，断言改为 `"zh-CN"`：

```rust
    #[test]
    fn deserializes_with_defaults() {
        let json = r#"{
            "targetLang": "zh-CN"
        }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("缺少字段应可反序列化")
            .normalized();
        assert_eq!(config.target_lang, "zh-CN");
        assert!(config.popup_precreate);
        assert!(config.overlay_precreate);
        assert!(config.collect_usage);
        assert!(config.services.is_empty());
        assert!(!config.is_configured());
    }
```

- [ ] **步骤 3：新增 `from_env_default_target_lang_is_zh_cn` 测试**（在 `deserializes_with_defaults` 之后或 `normalized_fills_ui_runtime_defaults` 之后加）

```rust
    #[test]
    fn from_env_default_target_lang_is_zh_cn() {
        let config = AppConfig::from_env();
        assert_eq!(config.target_lang, "zh-CN");
    }
```

- [ ] **步骤 4：检查 `deserializes_services_array` 测试**（types.rs:417）

该测试输入 JSON 含 `"targetLang": "中文"`（types.rs:419），但不断言 `target_lang`（只断言 services 字段）。**保留 "中文" 不动**（测试的是 services 数组反序列化，targetLang 是 mock 数据，保留无影响）。若执行者倾向统一，可改为 `"zh-CN"`，非强制。

- [ ] **步骤 5：运行测试验证通过**
  - `cd src-tauri && cargo test --lib config::types::tests`
  - 预期：PASS（含新增 1 个测试 + 改后的 deserializes_with_defaults + 原有全过）。

- [ ] **步骤 6：Commit**（PM 串行 commit）

### acceptance

- `cd src-tauri && cargo test --lib config::types::tests` 全绿。
- `cd src-tauri && cargo build` 通过。
- `AppConfig::from_env().target_lang == "zh-CN"`。
- `normalized` 的 `normalize_string(self.target_lang, DEFAULT_TARGET_LANG)` 回退值随之是 `"zh-CN"`（无需单独改 normalize_string 调用）。

---

## BE-3：SessionLanguages DTO + get/set command + 翻译入口读会话语言

- **task_id**：BE-3
- **owner**：backend
- **files_to_write**：
  - `src-tauri/src/ui/web_popup.rs`
- **files_to_read**：
  - `src-tauri/src/ui/web_popup.rs`（start_translation_from_input、现有 command 风格）
  - `src-tauri/src/app/state.rs`（BE-1 产出的 session_languages/set_session_languages 方法）
  - 主 plan 契约段 C-1 / C-2 / C-3 / C-5
- **contract_refs**：C-1（SessionLanguages DTO）、C-2（get_session_languages）、C-3（set_session_languages）、C-5（translation:event 不变）
- **depends_on**：BE-1（需 `AppState::session_languages` / `set_session_languages` 方法）
- **can_parallel_with**：BE-2、BE-5、FE-1、FE-2、FE-3
- **model_tier**：weak
- **tier_rationale**：默认弱模型。新增两个薄 command + 一处调用改值，照现有 command 模式（take_pending_source_text 等）编写。
- **boundary_rationale**：command 是 Tauri 传输适配边界（§5.1），承载 IPC 但不含业务逻辑。SessionLanguages 字段为代码（UI 无关）。翻译入口读会话语言是业务逻辑，归后端。

### 实现要点

- [ ] **步骤 1：定义 SessionLanguages DTO**

在 `web_popup.rs` 顶部（`pub const TRANSLATION_EVENT` 附近、`emit_translation_event` 之前或之后）加：

```rust
/// 会话语言 IPC DTO。字段为语言代码（如 "auto"/"zh-CN"），非显示名。
/// 序列化为 `{ sourceLang, targetLang }`。仅 Serialize（get 返回；set 用两个 String 参数）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLanguages {
    pub source_lang: String,
    pub target_lang: String,
}
```

- [ ] **步骤 2：新增两个 command**（在 `take_pending_source_text` 附近加，照现有 command 风格）

```rust
#[tauri::command]
pub async fn get_session_languages(
    state: tauri::State<'_, AppState>,
) -> Result<SessionLanguages, String> {
    let (source_lang, target_lang) = state.session_languages();
    Ok(SessionLanguages {
        source_lang,
        target_lang,
    })
}

#[tauri::command]
pub async fn set_session_languages(
    source_lang: String,
    target_lang: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.set_session_languages(source_lang, target_lang)
}
```

- [ ] **步骤 3：`start_translation_from_input` 改用会话语言**

`web_popup.rs:84-90` 当前：

```rust
    let requests = batch::build_batch_requests(
        input.clone(),
        config.target_lang.clone(),
        config.default_source_lang.clone(),
        &config.services,
        &batch_id,
    )?;
```

改为：

```rust
    let (session_source_lang, session_target_lang) = state.session_languages();
    let requests = batch::build_batch_requests(
        input.clone(),
        session_target_lang,
        session_source_lang,
        &config.services,
        &batch_id,
    )?;
```

> 注：`config` 仍需保留（后续 `config.services`、`config.collect_usage`、`config.log_level` 仍用到）。只把语言参数从 config 改为 session。

- [ ] **步骤 4：运行 cargo build 验证**
  - `cd src-tauri && cargo build`
  - 预期：BUILD SUCCEEDED。若报 `serialize` 缺 trait，确认 `serde::Serialize` derive 正确（不需要 `Deserialize`）。

- [ ] **步骤 5：运行全部测试确认无回归**
  - `cd src-tauri && cargo test`
  - 预期：PASS（翻译入口改值不影响 build_batch_requests 测试，因其传参不变）。

- [ ] **步骤 6：Commit**（PM 串行 commit）

### acceptance

- `cd src-tauri && cargo build` 通过。
- `cd src-tauri && cargo test` 全绿。
- `get_session_languages` / `set_session_languages` 两个 `#[tauri::command]` 存在且签名匹配 C-2/C-3。
- `start_translation_from_input` 内 `build_batch_requests` 调用使用 `state.session_languages()` 而非 `config.target_lang`/`config.default_source_lang`。
- `SessionLanguages` 仅 `Serialize`，`#[serde(rename_all = "camelCase")]`，字段为 snake_case Rust 名序列化为 camelCase。

---

## BE-4：lib.rs 注册两个 command

- **task_id**：BE-4
- **owner**：backend
- **files_to_write**：
  - `src-tauri/src/lib.rs`
- **files_to_read**：
  - `src-tauri/src/lib.rs`（`use ui::{...}` 块、`invoke_handler` 命令列表）
  - `src-tauri/src/ui/web_popup.rs`（BE-3 产出的 command 函数名）
  - 主 plan 契约段 C-2 / C-3
- **contract_refs**：C-2、C-3
- **depends_on**：BE-3（command 函数需先存在才能 import 与注册）
- **can_parallel_with**：BE-5、FE-1、FE-2、FE-3
- **model_tier**：weak
- **tier_rationale**：默认弱模型。两行注册（import + generate_handler 列表），机械改动。
- **boundary_rationale**：应用装配入口，归后端。无 UI 语义。

### 实现要点

- [ ] **步骤 1：`use ui::{...}` 块加 import**

`lib.rs:24-26` 当前：

```rust
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
```

改为：

```rust
    web_popup::{
        cancel_translation, get_session_languages, retry_translation, set_session_languages,
        start_translation, take_pending_source_text,
    },
```

- [ ] **步骤 2：`invoke_handler` 命令列表加两个**

`lib.rs:42-61` 的 `tauri::generate_handler![...]` 列表，在 `take_pending_source_text` 之后（或 `get_app_config` 之前，按相近性）加：

```rust
            get_session_languages,
            set_session_languages,
```

- [ ] **步骤 3：运行 cargo build 验证**
  - `cd src-tauri && cargo build`
  - 预期：BUILD SUCCEEDED。

- [ ] **步骤 4：Commit**（PM 串行 commit）

### acceptance

- `cd src-tauri && cargo build` 通过。
- `invoke_handler` 含 `get_session_languages`、`set_session_languages`。
- 前端 `invoke('get_session_languages')` / `invoke('set_session_languages', { sourceLang, targetLang })` 在 `npm run tauri dev` 下可调通（手动验证，依赖前端 FE-3 接入）。

---

## BE-5：tauri.conf.json main 加 skipTaskbar

- **task_id**：BE-5
- **owner**：backend
- **files_to_write**：
  - `src-tauri/tauri.conf.json`
- **files_to_read**：
  - `src-tauri/tauri.conf.json`（main 窗口配置）
  - 主 plan 实现澄清（skipTaskbar 是窗口配置非权限）
- **contract_refs**：无（窗口配置，非 IPC 契约）
- **depends_on**：-
- **can_parallel_with**：BE-1、BE-2、BE-3、BE-4、FE-1、FE-2、FE-3
- **model_tier**：weak
- **tier_rationale**：默认弱模型。单行 JSON 字段添加。
- **boundary_rationale**：`skipTaskbar` 是 Tauri 窗口配置（`tauri.conf.json` 的 `app.windows[].skipTaskbar`），**非权限**（反模式 §11.24：Tauri 2 权限只在 `capabilities/`）。窗口装配归后端。settings 窗口不动（保留任务栏图标，方便 Alt+Tab）。

### 实现要点

- [ ] **步骤 1：main 窗口加 skipTaskbar**

`tauri.conf.json` 的 `app.windows[0]`（label=main）当前：

```json
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
      }
```

加 `"skipTaskbar": true`（放在 `transparent` 与 `center` 之间，或 `center` 之后，顺序不影响）：

```json
      {
        "label": "main",
        "url": "translate.html",
        "title": "Shizi - 翻译助手",
        "width": 420,
        "height": 480,
        "resizable": false,
        "decorations": false,
        "transparent": true,
        "skipTaskbar": true,
        "center": true
      }
```

> **不要**在 `capabilities/default.json` 加任何 skipTaskbar 相关权限（该字段不存在于 capabilities，反模式 §11.24）。`capabilities/default.json` 本任务不修改。

- [ ] **步骤 2：运行 cargo build 验证配置编译期校验**
  - `cd src-tauri && cargo build`
  - 预期：BUILD SUCCEEDED（Tauri 2 WindowConfig 支持 `skipTaskbar: bool`，编译期校验通过）。

- [ ] **步骤 3：Commit**（PM 串行 commit）

### acceptance

- `cd src-tauri && cargo build` 通过。
- `npm run tauri dev` 启动后，main 翻译弹窗不在 Windows 任务栏显示图标；settings 窗口仍显示图标（未改）。
- `capabilities/default.json` 未被修改。

---

## 后端任务依赖与并行总览

```
BE-1 (state.rs) ─────> BE-3 (web_popup.rs) ──> BE-4 (lib.rs)
BE-2 (types.rs) ──────┐ (BE-2 与 BE-3 文件无交集，可并行；BE-3 依赖 BE-1)
BE-5 (tauri.conf.json) [独立]
```

- **串行**：BE-1 -> BE-3 -> BE-4
- **可并行**：BE-2、BE-5 全程独立；BE-2 可与 BE-3 并行（BE-1 完成后）
- **文件锁**：5 任务 5 文件，无交集
