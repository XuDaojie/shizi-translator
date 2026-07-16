# GitHub 检查更新（跳转下载）实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 从 GitHub Releases 检查新版本，设置页手动检查 + 启动自动检查（可关），有更新时提示并 `open_url` 跳转 Release 下载页；不做应用内安装。

**架构：** 核心纯函数（tag 解析、通道过滤、semver 比较）放 `core/update/`；`reqwest` 拉 `api.github.com`；`ui/update.rs` 暴露 `check_for_update` command。设置页走前端 toast/Dialog + `open_url`；启动自动检查走后端 `tauri-plugin-dialog` 系统消息框，确认后后端直接 `open_url`。不引入 `tauri-plugin-updater`，不改 release CI。

**技术栈：** Rust（`reqwest` + 新增 `semver`）、Tauri 2 commands、`tauri-plugin-dialog`、Vue 3 设置页、vitest / cargo test

**规格来源：** `docs/superpowers/specs/2026-07-16-github-check-update-design.md`

---

## 与 spec 的实现澄清（写死未决项）

1. **启动有更新时的对话框宿主（最终选型）**  
   - **启动路径**：后端 `tauri-plugin-dialog` 系统消息框（`MessageDialogButtons` 自定义「前往下载」/「稍后」），用户确认后**后端**调用既有 `open_url`（或同等 https 校验 + `cmd /C start`）。  
   - **设置页手动路径**：前端 toast + 现有 `Dialog` 组件；「前往下载」调 `invokeOpenUrl`。  
   - **禁止**：因检查更新强制 `show` 翻译弹窗；不做「settings 可见则前端 / 否则后端」双路径分叉。  
   - 理由：不依赖某 WebView 是否已挂载；与 spec §9.2 推荐路径一致。

2. **空候选集**  
   - 视为 `status: "up_to_date"`，`latestVersion: null`，`message` 可为 `null`（前端 toast 只展示「已是最新」即可）。

3. **文案语言**  
   - **后端**系统 dialog 与 `CheckUpdateResult.message`（error）：**中文产品常量**（与托盘中文硬编码策略一致，避免半英半中）。  
   - **前端**设置页 toast / Dialog：**i18n key**（8 语 locale 全补）。

4. **版本展示 API**  
   - **不**新增 `get_app_version` command。  
   - 设置页「当前版本」复用 AdvancedPanel 模式：`window.__TAURI__.app.getVersion()`。  
   - `check_for_update` 返回体仍含 `currentVersion`（`env!("CARGO_PKG_VERSION")`），供 toast/Dialog 与启动 dialog 使用。

5. **GitHub 仓库常量**（写死，不进配置）  
   - Owner/repo：`XuDaojie/shizi-translator`  
   - API：`https://api.github.com/repos/XuDaojie/shizi-translator/releases?per_page=30`  
   - 兜底下载页：`https://github.com/XuDaojie/shizi-translator/releases`  
   - 超时：12 秒  
   - UA：`shizi/<CARGO_PKG_VERSION>`  
   - Accept：`application/vnd.github+json`  
   - 无 token

6. **通道规则（再强调）**  
   - `stable`：`prerelease == false` **且** semver 无 pre-release 段  
   - `beta`：全部非 draft、可解析 tag（含正式版与 pre）  
   - 候选中取 semver **最高** 一条  
   - `latest > current` → `update_available`；否则 `up_to_date`

7. **capabilities**  
   - 后端 `DialogExt` 系统消息框从 Rust 调用；参考既有导出日志结论，**后端调 dialog 通常不需要 capability**。本计划默认**不**加 `dialog:allow-ask`；若实机启动 dialog 被拒再补。  
   - `open_url` 已是 command，无新增权限。

8. **本轮明确不做**  
   - `tauri-plugin-updater`、应用内下载安装、改 CI / bump 脚本、24h 定时检查、自定义仓库 URL、强制更新、`pot-desktop` 对照实现

---

## 文件结构

| 文件 | 职责 |
|---|---|
| 修改 `src-tauri/Cargo.toml` | 加依赖 `semver = "1"` |
| 创建 `src-tauri/src/core/update/mod.rs` | 模块导出 |
| 创建 `src-tauri/src/core/update/types.rs` | `UpdateChannel`、`ReleaseInfo`、`CheckUpdateStatus`、`CheckUpdateResult` |
| 创建 `src-tauri/src/core/update/version.rs` | tag 解析、通道过滤、选 latest、与 current 比较（纯函数 + 单测） |
| 创建 `src-tauri/src/core/update/github.rs` | HTTP 拉 releases + 映射错误（可注入 base URL 便于测；默认常量） |
| 修改 `src-tauri/src/core/mod.rs` | `pub mod update;` |
| 修改 `src-tauri/src/core/config/types.rs` | `AppConfig` 加 `update_channel` / `auto_check_update` + default + normalized |
| 创建 `src-tauri/src/ui/update.rs` | `check_for_update` command；`spawn_startup_update_check` |
| 修改 `src-tauri/src/ui/mod.rs` | `pub mod update;` |
| 修改 `src-tauri/src/lib.rs` | 注册 command；setup 末尾异步启动检查 |
| 修改 `frontend/src/types/config.ts` | `AppConfig` 两字段 |
| 修改 `frontend/src/lib/config.ts` | `projectToAppConfig` 投影 |
| 修改 `frontend/src/lib/config.test.ts` | 投影断言 |
| 修改 `frontend/src/lib/tauri.ts` | `invokeCheckForUpdate` + 类型 |
| 修改 `frontend/src/settings/stores/settings.ts` | `syncFromBackend` 合并两字段 |
| 修改 `frontend/src/settings/stores/settings.test.ts` | `makeAppConfig` 补字段；merge 相关断言（若有） |
| 修改 `frontend/src/settings/panels/GeneralPanel.vue` | 去掉 autoCheckUpdate 的 DevOnly/wip；版本 + 检查按钮 + Dialog |
| 修改 `frontend/src/i18n/locales/*.json`（8 个） | 新 key + 修正 autoCheck 描述 |
| 修改 `README.md` / `docs/roadmap/progressive-development-plan.md` / `AGENTS.md` / `CLAUDE.md` / spec 状态 | 实现收尾文档（任务 9） |

**刻意不改：**  
- `.github/workflows/release.yml`、`scripts/bump-version.js`  
- `AdvancedPanel` 关于区版本展示（可并存）  
- `pot-desktop/`  

---

## 任务 1：后端 `AppConfig` 更新字段（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 测试：同文件 `#[cfg(test)]`

- [ ] **步骤 1：编写失败的测试**

在 `types.rs` 的 tests 模块追加：

```rust
#[test]
fn app_config_defaults_update_fields() {
    let config = AppConfig::default();
    assert_eq!(config.update_channel, "stable");
    assert!(config.auto_check_update);
}

#[test]
fn app_config_missing_update_fields_deserialize_to_defaults() {
    let json = r#"{
        "targetLang": "zh-CN",
        "services": [],
        "ocrServices": []
    }"#;
    let config: AppConfig = serde_json::from_str(json).expect("deserialize");
    let config = config.normalized();
    assert_eq!(config.update_channel, "stable");
    assert!(config.auto_check_update);
}

#[test]
fn app_config_normalized_rejects_invalid_update_channel() {
    let mut config = AppConfig::default();
    config.update_channel = "nightly".into();
    let config = config.normalized();
    assert_eq!(config.update_channel, "stable");
}
```

- [ ] **步骤 2：运行测试确认失败**

```bash
cd src-tauri && cargo test app_config_defaults_update_fields app_config_missing_update_fields_deserialize_to_defaults app_config_normalized_rejects_invalid_update_channel -- --nocapture
```

预期：编译失败（字段不存在）或 FAIL。

- [ ] **步骤 3：最少实现**

在 `AppConfig` 结构体（`#[serde(rename_all = "camelCase")]`）增加：

```rust
#[serde(default = "default_update_channel")]
pub update_channel: String,
#[serde(default = "default_true")]
pub auto_check_update: bool,
```

辅助函数：

```rust
fn default_update_channel() -> String {
    "stable".to_string()
}

fn normalize_update_channel(value: String) -> String {
    match value.trim() {
        "beta" => "beta".to_string(),
        _ => "stable".to_string(),
    }
}
```

- `AppConfig::default()` 填：`update_channel: default_update_channel()`，`auto_check_update: true`  
- `normalized()` 末尾：`self.update_channel = normalize_update_channel(self.update_channel);`  
- **不要**改动其他字段语义

- [ ] **步骤 4：运行测试确认通过**

```bash
cd src-tauri && cargo test app_config_defaults_update_fields app_config_missing_update_fields_deserialize_to_defaults app_config_normalized_rejects_invalid_update_channel -- --nocapture
```

预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): AppConfig 增加 updateChannel 与 autoCheckUpdate"
```

---

## 任务 2：前端配置投影与同步（TDD）

**文件：**
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`（`makeAppConfig` 等凡构造 `AppConfig` 处）

- [ ] **步骤 1：编写失败的测试**

在 `config.test.ts` 的 `projectToAppConfig` describe 中追加：

```typescript
it('投影 updateChannel 与 autoCheckUpdate', () => {
  const state = makeState([]);
  state.general.updateChannel = 'beta';
  state.general.autoCheckUpdate = false;
  const config = projectToAppConfig(state);
  expect(config.updateChannel).toBe('beta');
  expect(config.autoCheckUpdate).toBe(false);
});
```

在 `validateConfig` 的 `base` 对象与所有内联 `AppConfig` 字面量中补：

```typescript
updateChannel: 'stable',
autoCheckUpdate: true,
```

（否则 TS 会因缺字段报错——若先改类型再写测，按 TDD 顺序：先加测与类型，再改投影。）

- [ ] **步骤 2：运行测试确认失败**

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：FAIL（`config.updateChannel` undefined）或 typecheck 失败。

- [ ] **步骤 3：实现类型、投影、同步**

1. `frontend/src/types/config.ts` 的 `AppConfig` 增加：

```typescript
updateChannel: 'stable' | 'beta';
autoCheckUpdate: boolean;
```

2. `projectToAppConfig` 返回对象增加：

```typescript
updateChannel: state.general.updateChannel,
autoCheckUpdate: state.general.autoCheckUpdate,
```

3. `settings.ts` 的 `syncFromBackend` 在 `state.general.language = ...` 附近增加：

```typescript
state.general.updateChannel =
  backend.updateChannel === 'beta' ? 'beta' : 'stable'
state.general.autoCheckUpdate =
  backend.autoCheckUpdate ?? state.general.autoCheckUpdate
```

（后端非空 merge 路径；空 services 推送路径已走 `projectToAppConfig`，字段会随投影写入。）

4. 修复 `settings.test.ts` 中 `makeAppConfig` 与所有 `AppConfig` 字面量，补两字段默认值，保证现有测试编译通过。

- [ ] **步骤 4：运行测试确认通过**

```bash
npm run test -- frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.test.ts
npm run typecheck
```

预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/types/config.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(settings): 前后端同步 updateChannel 与 autoCheckUpdate"
```

---

## 任务 3：`semver` 依赖 + 纯函数版本/通道逻辑（TDD）

**文件：**
- 修改：`src-tauri/Cargo.toml`（`semver = "1"`）
- 修改：`src-tauri/src/core/mod.rs`（`pub mod update;`）
- 创建：`src-tauri/src/core/update/mod.rs`
- 创建：`src-tauri/src/core/update/types.rs`
- 创建：`src-tauri/src/core/update/version.rs`

- [ ] **步骤 1：编写失败的测试（先建 `version.rs` 仅含 tests 调用的 API 签名占位亦可，推荐 tests 与实现同文件）**

`version.rs` 中 `#[cfg(test)] mod tests` 写全：

```rust
use super::*;
use crate::core::update::types::{ReleaseInfo, UpdateChannel};

#[test]
fn parse_tag_strips_v_prefix() {
    assert_eq!(parse_tag_version("v1.2.3").unwrap().to_string(), "1.2.3");
    assert_eq!(parse_tag_version("1.2.3").unwrap().to_string(), "1.2.3");
    assert_eq!(
        parse_tag_version("v0.7.0-beta.5").unwrap().to_string(),
        "0.7.0-beta.5"
    );
    assert!(parse_tag_version("not-a-version").is_none());
    assert!(parse_tag_version("").is_none());
}

#[test]
fn stable_channel_skips_prerelease_flag_and_semver_pre() {
    let releases = vec![
        ReleaseInfo {
            tag_name: "v0.8.0-beta.1".into(),
            name: Some("beta".into()),
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0-beta.1".into(),
            prerelease: true,
            draft: false,
        },
        ReleaseInfo {
            tag_name: "v0.7.1".into(),
            name: Some("stable".into()),
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.1".into(),
            prerelease: false,
            draft: false,
        },
        ReleaseInfo {
            tag_name: "bad-tag".into(),
            name: None,
            html_url: "https://github.com/example/x".into(),
            prerelease: false,
            draft: false,
        },
    ];
    let latest = select_latest_for_channel(&releases, UpdateChannel::Stable).unwrap();
    assert_eq!(latest.version.to_string(), "0.7.1");
    assert!(!latest.is_prerelease);
}

#[test]
fn beta_channel_picks_highest_including_prerelease() {
    let releases = vec![
        ReleaseInfo {
            tag_name: "v0.7.0".into(),
            name: None,
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.0".into(),
            prerelease: false,
            draft: false,
        },
        ReleaseInfo {
            tag_name: "v0.7.0-beta.9".into(),
            name: None,
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.7.0-beta.9".into(),
            prerelease: true,
            draft: false,
        },
        ReleaseInfo {
            tag_name: "v0.8.0-beta.1".into(),
            name: None,
            html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v0.8.0-beta.1".into(),
            prerelease: true,
            draft: false,
        },
    ];
    // semver: 0.8.0-beta.1 > 0.7.0 > 0.7.0-beta.9
    let latest = select_latest_for_channel(&releases, UpdateChannel::Beta).unwrap();
    assert_eq!(latest.version.to_string(), "0.8.0-beta.1");
    assert!(latest.is_prerelease);
}

#[test]
fn compare_update_available_when_latest_greater() {
    assert!(is_update_available("0.7.0", "0.7.1"));
    assert!(is_update_available("0.7.0", "0.7.0-beta.6") == false); // 0.7.0 > 0.7.0-beta.6
    assert!(!is_update_available("0.7.0", "0.7.0"));
    assert!(is_update_available("0.7.0-beta.5", "0.7.0-beta.6"));
}

#[test]
fn draft_releases_are_ignored() {
    let releases = vec![ReleaseInfo {
        tag_name: "v9.0.0".into(),
        name: None,
        html_url: "https://github.com/XuDaojie/shizi-translator/releases/tag/v9.0.0".into(),
        prerelease: false,
        draft: true,
    }];
    assert!(select_latest_for_channel(&releases, UpdateChannel::Stable).is_none());
}
```

注意：`is_update_available("0.7.0", "0.7.0-beta.6")` 按 semver 正式版 **高于** 同号 pre；断言必须符合 `semver` crate 序，勿写反。

- [ ] **步骤 2：运行确认失败**

```bash
cd src-tauri && cargo test --package shizi parse_tag_strips_v_prefix -- --nocapture
```

预期：编译失败或 FAIL。

- [ ] **步骤 3：实现**

`Cargo.toml`：

```toml
semver = "1"
```

`types.rs` 关键类型（camelCase 序列化供 command 返回）：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "beta" => Self::Beta,
            _ => Self::Stable,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone)]
pub struct SelectedRelease {
    pub version: semver::Version,
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub is_prerelease: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckUpdateStatus {
    UpToDate,
    UpdateAvailable,
    Error,
}

// 前端约定 camelCase：用 rename_all = "camelCase" 在结果 struct 上
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckUpdateResult {
    pub status: String, // "up_to_date" | "update_available" | "error"
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_name: Option<String>,
    pub release_url: Option<String>,
    pub is_prerelease: Option<bool>,
    pub message: Option<String>,
}
```

**status 字符串约定（与 spec 一致，勿用 PascalCase）：**

```text
"up_to_date" | "update_available" | "error"
```

`version.rs` 实现要点：

```rust
pub fn parse_tag_version(tag: &str) -> Option<semver::Version> {
    let t = tag.trim().strip_prefix('v').unwrap_or(tag.trim());
    semver::Version::parse(t).ok()
}

pub fn select_latest_for_channel(
    releases: &[ReleaseInfo],
    channel: UpdateChannel,
) -> Option<SelectedRelease> { /* filter draft; parse; channel; max_by version */ }

pub fn is_update_available(current: &str, latest: &str) -> bool {
    match (parse_tag_version(current), parse_tag_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

pub fn evaluate_check(
    current_version: &str,
    releases: &[ReleaseInfo],
    channel: UpdateChannel,
) -> CheckUpdateResult { /* 空候选 → up_to_date；比较后填字段 */ }
```

`mod.rs`：

```rust
pub mod github;
pub mod types;
pub mod version;

pub use types::*;
pub use version::*;
```

（`github` 可在任务 4 再补文件；若任务 3 先不建 `github.rs`，则 `mod.rs` 暂只 export version/types。）

- [ ] **步骤 4：测试通过**

```bash
cd src-tauri && cargo test --package shizi parse_tag_strips_v_prefix stable_channel_skips_prerelease_flag_and_semver_pre beta_channel_picks_highest_including_prerelease compare_update_available_when_latest_greater draft_releases_are_ignored -- --nocapture
```

预期：全部 PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/core/mod.rs src-tauri/src/core/update/
git commit -m "feat(update): 版本解析与通道过滤纯函数"
```

---

## 任务 4：GitHub client + `check_for_update` command

**文件：**
- 创建：`src-tauri/src/core/update/github.rs`
- 创建：`src-tauri/src/ui/update.rs`
- 修改：`src-tauri/src/core/update/mod.rs`（export github）
- 修改：`src-tauri/src/ui/mod.rs`
- 修改：`src-tauri/src/lib.rs`（handler 注册）

- [ ] **步骤 1：实现 `github.rs`（纯逻辑错误映射 + 可选单测 mock JSON 解析）**

常量：

```rust
pub const GITHUB_OWNER_REPO: &str = "XuDaojie/shizi-translator";
pub const RELEASES_API_URL: &str =
    "https://api.github.com/repos/XuDaojie/shizi-translator/releases?per_page=30";
pub const RELEASES_PAGE_FALLBACK: &str =
    "https://github.com/XuDaojie/shizi-translator/releases";
const REQUEST_TIMEOUT_SECS: u64 = 12;
```

```rust
pub async fn fetch_releases(client: &reqwest::Client) -> Result<Vec<ReleaseInfo>, String> {
    let version = env!("CARGO_PKG_VERSION");
    let resp = client
        .get(RELEASES_API_URL)
        .header("User-Agent", format!("shizi/{version}"))
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| map_network_error(&e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(map_http_status(status.as_u16()));
    }

    resp.json::<Vec<ReleaseInfo>>()
        .await
        .map_err(|_| "无法解析 GitHub 响应".to_string())
}

fn map_http_status(code: u16) -> String {
    match code {
        403 | 429 => "GitHub 请求过于频繁或被拒绝，请稍后重试".into(),
        404 => "未找到发布信息".into(),
        s if s >= 500 => "GitHub 服务暂时不可用，请稍后重试".into(),
        _ => format!("检查更新失败（HTTP {code}）"),
    }
}

fn map_network_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "检查更新超时，请检查网络后重试".into()
    } else {
        "网络不可用，无法检查更新".into()
    }
}
```

单测（可选但推荐）：对 `map_http_status` / `map_network_error` 做纯函数测；**不要**在 CI 打真 GitHub。

在 `version` 或 `github` 加：

```rust
#[test]
fn map_http_status_rate_limit() {
    assert!(map_http_status(429).contains("稍后"));
}
```

（若 `map_http_status` 为 private，同文件测。）

- [ ] **步骤 2：`ui/update.rs` command**

```rust
use crate::{
    app::state::AppState,
    core::update::{
        evaluate_check, fetch_releases, UpdateChannel, CheckUpdateResult, RELEASES_PAGE_FALLBACK,
    },
};
use reqwest::Client;

#[tauri::command]
pub async fn check_for_update(
    channel: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<CheckUpdateResult, String> {
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let channel = UpdateChannel::parse(
        channel
            .as_deref()
            .unwrap_or(config.update_channel.as_str()),
    );
    let current = env!("CARGO_PKG_VERSION").to_string();

    log::info!(
        "检查更新开始 channel={} current={}",
        channel.as_str(),
        current
    );

    let client = Client::new();
    let result = match fetch_releases(&client).await {
        Ok(releases) => {
            log::debug!("GitHub 返回 release 数: {}", releases.len());
            evaluate_check(&current, &releases, channel)
        }
        Err(message) => {
            log::warn!("检查更新失败: {}", message);
            CheckUpdateResult {
                status: "error".into(),
                current_version: current,
                latest_version: None,
                release_name: None,
                release_url: None,
                is_prerelease: None,
                message: Some(message),
            }
        }
    };

    log::info!(
        "检查更新结束 status={} latest={:?}",
        result.status,
        result.latest_version
    );
    Ok(result)
}
```

`evaluate_check` 须保证：

- `update_available` 时 `release_url` 优先 `html_url`（若非 `https://` 则改用 `RELEASES_PAGE_FALLBACK`）  
- `error` 时 `message` 必填  

- [ ] **步骤 3：注册模块与 command**

`ui/mod.rs`：`pub mod update;`

`lib.rs`：

```rust
use ui::update::check_for_update;
// generate_handler 中加入 check_for_update
```

- [ ] **步骤 4：编译与单测**

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：通过。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/update/ src-tauri/src/ui/update.rs src-tauri/src/ui/mod.rs src-tauri/src/lib.rs
git commit -m "feat(update): 实现 check_for_update command 与 GitHub 客户端"
```

---

## 任务 5：启动自动检查 + 系统 dialog

**文件：**
- 修改：`src-tauri/src/ui/update.rs`（增加 `spawn_startup_update_check`）
- 修改：`src-tauri/src/lib.rs`（setup 调用）
- 可选：复用 `crate::ui::config::open_url`（注意 `open_url` 是 command 函数，可直接在同 crate 调用）

- [ ] **步骤 1：实现启动调度**

```rust
/// setup 完成后调用：best-effort，不阻塞。
pub fn spawn_startup_update_check(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let config = match state.config_store.get() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("启动检查更新：读取配置失败: {e}");
                return;
            }
        };
        if !config.auto_check_update {
            log::info!("启动检查更新：已关闭，跳过");
            return;
        }

        let channel = UpdateChannel::parse(&config.update_channel);
        let current = env!("CARGO_PKG_VERSION").to_string();
        let client = Client::new();
        let result = match fetch_releases(&client).await {
            Ok(releases) => evaluate_check(&current, &releases, channel),
            Err(message) => {
                log::warn!("启动检查更新失败（不打扰用户）: {message}");
                return;
            }
        };

        if result.status != "update_available" {
            log::info!("启动检查更新: {}", result.status);
            return;
        }

        let latest = result.latest_version.clone().unwrap_or_default();
        let url = result
            .release_url
            .clone()
            .unwrap_or_else(|| RELEASES_PAGE_FALLBACK.to_string());
        let pre = result.is_prerelease.unwrap_or(false);
        let body = if pre {
            format!(
                "发现新版本 {latest}（预发布）。\n当前版本 {}。\n是否前往下载页？",
                result.current_version
            )
        } else {
            format!(
                "发现新版本 {latest}。\n当前版本 {}。\n是否前往下载页？",
                result.current_version
            )
        };

        // 系统对话框必须在可与 UI 交互的上下文；blocking_show 在 async 任务中：
        // 用 spawn_blocking 避免卡住 runtime（Windows 消息框）。
        let app2 = app.clone();
        let url2 = url.clone();
        let go = tokio::task::spawn_blocking(move || {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            app2.dialog()
                .message(body)
                .title("发现新版本")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "前往下载".to_string(),
                    "稍后".to_string(),
                ))
                .blocking_show()
        })
        .await
        .unwrap_or(false);

        if go {
            if let Err(e) = crate::ui::config::open_url(url2) {
                log::warn!("启动检查更新：打开下载页失败: {e}");
            }
        }
    });
}
```

**注意：** 核对当前 `tauri-plugin-dialog` 版本的 `MessageDialogButtons::OkCancelCustom` 签名（可能是 `OkCancelCustom(&'static str, &'static str)` 或 `String`）。以编译器与 docs.rs 为准；若仅支持静态 str，用 `"前往下载"` / `"稍后"` 字面量。

若 `OkCancelCustom` 不可用，退化为：

```rust
.buttons(MessageDialogButtons::OkCancel)
// 文案靠 title/message 说明 OK=前往下载
```

- [ ] **步骤 2：在 `lib.rs` setup 末尾调用**

在 `ensure_popup_window` / `ensure_overlay` 之后、`Ok(())` 之前：

```rust
crate::ui::update::spawn_startup_update_check(app.handle().clone());
```

- [ ] **步骤 3：编译**

```bash
cd src-tauri && cargo build
cd src-tauri && cargo test
```

预期：通过。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/update.rs src-tauri/src/lib.rs
git commit -m "feat(update): 启动自动检查更新并用系统对话框提示"
```

---

## 任务 6：前端 invoke + i18n

**文件：**
- 修改：`frontend/src/lib/tauri.ts`
- 修改：`frontend/src/i18n/locales/zh-CN.json`
- 修改：`frontend/src/i18n/locales/en-US.json`
- 修改：其余 6 个 locale（`zh-TW` `ja-JP` `ko-KR` `fr-FR` `de-DE` `es-ES`）

- [ ] **步骤 1：`tauri.ts` 增加**

```typescript
export type CheckUpdateStatus = 'up_to_date' | 'update_available' | 'error'

export interface CheckUpdateResult {
  status: CheckUpdateStatus
  currentVersion: string
  latestVersion: string | null
  releaseName: string | null
  releaseUrl: string | null
  isPrerelease: boolean | null
  message: string | null
}

/** channel 缺省时后端读 AppConfig.updateChannel */
export async function invokeCheckForUpdate(channel?: 'stable' | 'beta'): Promise<CheckUpdateResult> {
  return requireInvoke()<CheckUpdateResult>('check_for_update', channel ? { channel } : {})
}
```

- [ ] **步骤 2：i18n key（zh-CN / en-US 必准；其它语言可先英/中合理翻译）**

新增（写入各 locale 的 `messages`）：

| key | zh-CN | en-US |
|---|---|---|
| `settings.button.checkUpdate` | 检查更新 | Check for updates |
| `settings.field.currentAppVersion` | 当前版本 | Current version |
| `settings.description.autoCheckUpdate` | **替换**为：启动时自动检查是否有新版本。 | Check for updates when the app starts. |
| `settings.toast.upToDate` | 已是最新版本（{version}） | You're up to date ({version}) |
| `settings.toast.checkUpdateFailed` | 检查更新失败 | Failed to check for updates |
| `settings.dialog.updateTitle` | 发现新版本 | Update available |
| `settings.dialog.updateDescription` | 最新版本 {latest}{prerelease}。当前版本 {current}。是否前往下载页？ | Version {latest}{prerelease} is available. You have {current}. Open the download page? |
| `settings.dialog.updatePrereleaseSuffix` | （预发布） |  (pre-release) |
| `settings.button.goDownload` | 前往下载 | Download |
| `settings.button.later` | 稍后 | Later |

`description.autoCheckUpdate` 旧文案「每隔 24 小时…」必须改掉（本轮无 24h 定时）。

- [ ] **步骤 3：Commit**

```bash
git add frontend/src/lib/tauri.ts frontend/src/i18n/locales/
git commit -m "feat(update): 前端 check_for_update 封装与 i18n 文案"
```

---

## 任务 7：设置页「更新」组 UI

**文件：**
- 修改：`frontend/src/settings/panels/GeneralPanel.vue`

- [ ] **步骤 1：改造更新组**

要求：

1. 去掉包裹 `autoCheckUpdate` 的 `<DevOnly>` 与 `status="wip"`。  
2. 增加只读「当前版本」行：与 AdvancedPanel 相同，`onMounted` 调 `getVersion`，展示 `v{version}`。  
3. 增加「检查更新」按钮：`checking` 时 `disabled` + `RefreshCw` 可 `animate-spin`。  
4. 手动检查逻辑：

```typescript
import { Dialog } from '@/components/ui/dialog'
import { invokeCheckForUpdate, invokeOpenUrl } from '@/lib/tauri'
import { toast } from '@/lib/toast'
import { ref, onMounted } from 'vue'
// ...

const checking = ref(false)
const updateDialogOpen = ref(false)
const pendingUpdate = ref<CheckUpdateResult | null>(null)
const appVersion = ref('…')

onMounted(async () => {
  try {
    const tauri = (window as unknown as {
      __TAURI__?: { app?: { getVersion?: () => Promise<string> } }
    }).__TAURI__
    const v = await tauri?.app?.getVersion?.()
    if (v) appVersion.value = v
  } catch { /* vite only */ }
})

async function handleCheckUpdate() {
  if (checking.value) return
  checking.value = true
  try {
    const result = await invokeCheckForUpdate(state.general.updateChannel)
    if (result.status === 'up_to_date') {
      toast.success(t('settings.toast.upToDate', { version: result.currentVersion }))
    } else if (result.status === 'update_available') {
      pendingUpdate.value = result
      updateDialogOpen.value = true
    } else {
      toast.error(
        t('settings.toast.checkUpdateFailed'),
        result.message ?? '',
      )
    }
  } catch (e) {
    toast.error(t('settings.toast.checkUpdateFailed'), String(e))
  } finally {
    checking.value = false
  }
}

async function goDownload() {
  const url = pendingUpdate.value?.releaseUrl
  updateDialogOpen.value = false
  if (!url) return
  try {
    await invokeOpenUrl(url)
  } catch (e) {
    toast.error(t('settings.toast.openUrlFailed'), String(e))
  }
}
```

模板「更新」组结构示意：

```vue
<SettingGroup :title="t('settings.group.update')" :description="t('settings.description.update')">
  <SettingRow :title="t('settings.field.updateChannel')" :description="t('settings.description.updateChannel')">
    <SettingSelect v-model="state.general.updateChannel" :options="updateChannelOptions" />
  </SettingRow>
  <SettingRow :title="t('settings.field.autoCheckUpdate')" :description="t('settings.description.autoCheckUpdate')">
    <SettingSwitch v-model="state.general.autoCheckUpdate" :aria-label="t('settings.field.autoCheckUpdate')" />
  </SettingRow>
  <SettingRow :title="t('settings.field.currentAppVersion')" :description="t('settings.description.version')">
    <span class="text-sm text-muted-foreground font-mono">v{{ appVersion }}</span>
  </SettingRow>
  <SettingRow :title="t('settings.button.checkUpdate')" description="">
    <Button size="sm" :disabled="checking" @click="handleCheckUpdate">
      <RefreshCw :class="['h-3.5 w-3.5', checking && 'animate-spin']" />
      {{ t('settings.button.checkUpdate') }}
    </Button>
  </SettingRow>
  <Dialog
    v-model:open="updateDialogOpen"
    :title="t('settings.dialog.updateTitle')"
    :description="updateDialogDescription"
    width="420px"
  >
    <div class="flex justify-end gap-2">
      <Button variant="ghost" @click="updateDialogOpen = false">{{ t('settings.button.later') }}</Button>
      <Button @click="goDownload">{{ t('settings.button.goDownload') }}</Button>
    </div>
  </Dialog>
</SettingGroup>
```

`updateDialogDescription` 用 `computed`，基于 `pendingUpdate` 拼 `t('settings.dialog.updateDescription', { latest, current, prerelease })`。

注意：`defineProps` 若用 `state`，script 内检查更新需 `const props = defineProps<...>()` 或保留解构；`updateChannel` 传 `props.state.general.updateChannel`。

`Dialog` 无 trigger 时参考 HistoryPanel：仅 `v-model:open` 控制（无 `#trigger` 亦可，确认 `Dialog.vue` 对无 trigger 的支持；若不支持，用 `resetOpen` 同款结构或 hidden trigger）。以现有 `Dialog.vue` 为准——若必须 trigger，放一个 `class="hidden"` 的 span 作 trigger，或直接 `updateDialogOpen = true` 若 Root 支持受控。

- [ ] **步骤 2：类型检查**

```bash
npm run typecheck
```

预期：通过。

- [ ] **步骤 3：Commit**

```bash
git add frontend/src/settings/panels/GeneralPanel.vue
git commit -m "feat(settings): 更新组接通检查更新与自动检查开关"
```

---

## 任务 8：全量验证

- [ ] **步骤 1：后端**

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：全部 PASS / 编译成功。

- [ ] **步骤 2：前端**

```bash
npm run test
npm run typecheck
```

预期：PASS。

- [ ] **步骤 3：手动验收清单（开发者本机）**

| # | 步骤 | 期望 |
|---|---|---|
| 1 | 稳定通道 + 当前已最新 → 设置页检查 | toast 已最新 |
| 2 | 临时把比较基准改低或指向更高 tag 的 mock/本地改 `CARGO_PKG_VERSION` 测 | Dialog → 前往下载打开正确 Release |
| 3 | 切 beta，若有更高 pre | 能提示预发布 |
| 4 | 关自动检查后重启 | 无系统对话框 |
| 5 | 断网手动检查 | toast 失败；启动无骚扰 |
| 6 | 确认未引入 updater 插件、未改 release.yml | git diff 无相关文件 |

- [ ] **步骤 4：若手动发现问题则修并补测后小 commit**

---

## 任务 9：文档同步（收尾硬门禁）

**文件：**
- `README.md`：下载安装旁补一句应用内「检查更新 → 浏览器下载」  
- `docs/roadmap/progressive-development-plan.md`：检查更新完成；自动检查非 wip  
- `AGENTS.md` 与 `CLAUDE.md`（**必须同步**）：  
  - `AppConfig` 含 `updateChannel` / `autoCheckUpdate`  
  - command `check_for_update`  
  - 启动 `autoCheckUpdate` 时系统 dialog  
  - 从 wip / DevOnly 列表去掉「自动检查更新」  
- `docs/superpowers/specs/2026-07-16-github-check-update-design.md`：状态改为「已实现」  
- 本 plan 任务复选框回填

- [ ] **步骤 1：按协作规范改文档**

- [ ] **步骤 2：Commit**

```bash
git add README.md docs/roadmap/progressive-development-plan.md AGENTS.md CLAUDE.md docs/superpowers/specs/2026-07-16-github-check-update-design.md docs/superpowers/plans/2026-07-16-github-check-update.md
git commit -m "docs(update): 同步检查更新能力说明与规格状态"
```

---

## 自检（对照 spec）

| Spec 章节 | 对应任务 |
|---|---|
| §2 范围内：GitHub 检查、command、配置、设置页、启动检查、dialog、单测、文档 | 1–9 |
| §2 范围外：updater/CI/24h/自定义仓库 | 澄清 §8 刻意不改 |
| §6 配置字段与同步 | 任务 1–2 |
| §7 版本/通道/比较 | 任务 3 |
| §8 Command 契约 | 任务 4 |
| §9.1 设置页 | 任务 6–7 |
| §9.2 启动检查 + 宿主选型 | 任务 5（系统 dialog 写死） |
| §9.3 i18n | 任务 6 |
| §10–11 错误与安全 | 任务 4–5（https、中文 message、日志等级） |
| §12 测试 | 任务 1–3、8 |
| §13 不改 CI | 全程 |
| §14 文档 | 任务 9 |
| §16 成功标准 | 任务 8–9 |

**类型一致性：**

- 配置：`updateChannel` / `autoCheckUpdate`（前端 camelCase）= `update_channel` / `auto_check_update`（Rust + serde camelCase）  
- 结果：`status` 字符串字面量 `up_to_date` | `update_available` | `error`  
- Command 名：`check_for_update`  
- 通道：仅 `stable` | `beta`

**占位符扫描：** 无 TODO/待定；dialog 按钮 API 以编译器为准已给降级路径。

---

## 风险备忘（实现时）

1. `MessageDialogButtons` API 因 plugin 小版本可能不同 → 编译不过时查 `tauri-plugin-dialog` 文档，用 OkCancel 降级。  
2. `spawn_blocking` + `blocking_show` 在 headless CI 无 UI；单测勿调用 dialog。  
3. 存量 `config.json` 缺字段靠 serde default，勿写迁移脚本。  
4. `syncFromBackend` 过去未合并 `popupPrecreate` 等字段——本任务**只**补更新相关两字段，不顺手扩 scope。
