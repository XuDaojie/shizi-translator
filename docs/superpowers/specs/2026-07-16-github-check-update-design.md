# GitHub 检查更新（跳转下载）设计规格

- 日期：2026-07-16
- 状态：已批准（待实现）
- 关联：
  - 设置页已有 UI 占位：`GeneralPanel`「更新」组（`updateChannel` / `autoCheckUpdate`，后者现为 `DevOnly` + `status="wip"`）
  - 现有 `open_url` command（`src-tauri/src/ui/config.rs`）
  - GitHub 仓库与发版：`XuDaojie/shizi-translator`，`.github/workflows/release.yml` 已发布 NSIS 到 Releases
  - 前端本地类型：`frontend/src/settings/types.ts`（`UpdateChannel`、`GeneralSettings`）
  - 后端配置：`src-tauri/src/core/config/types.rs`（`AppConfig`，**当前无**更新相关字段）

## 1. 目的

为 Windows 端 shizi 增加**从 GitHub Releases 检查新版本**的能力：发现更新后提示用户，确认后用系统浏览器打开 Release 下载页，用户自行下载 NSIS 安装包升级。

本轮**不做**应用内静默下载、签名校验或替换安装（不引入 `tauri-plugin-updater`）。

## 2. 范围

### 范围内

- 后端通过 GitHub Releases API 拉取 release 列表，按**更新通道**过滤并做 **semver** 比较
- 新增 Tauri command `check_for_update`（及必要的版本/比较纯函数）
- `AppConfig` 持久化 `updateChannel`、`autoCheckUpdate`，前后端同步
- 设置页「更新」组：
  - 更新通道（stable / beta）接后端
  - 自动检查更新：去掉 `DevOnly` / `wip`，接后端
  - 展示当前应用版本
  - 手动「检查更新」按钮
- 应用启动后：若 `autoCheckUpdate == true`，异步 best-effort 检查一次；仅在有可用更新时提示
- 有更新时对话框：展示最新版本信息 +「前往下载」/「稍后」；前往下载调用已有 `open_url`
- 单元测试：版本解析/比较、通道过滤、配置默认值与投影
- 文档：本 spec；实现计划另文；实现后同步 README / roadmap 当前能力说明

### 范围外（YAGNI）

- 应用内下载安装包、自动安装、重启替换（`tauri-plugin-updater` / 签名密钥 / `latest.json`）
- 修改 `.github/workflows/release.yml` 或发版产物格式（**本轮不改 CI**）
- 修改 `scripts/bump-version.js`
- 托盘常驻期间每 24 小时定时检查（现有 i18n 文案可保留或弱化；本轮仅「启动 + 手动」）
- 自定义仓库 URL / 镜像源配置
- macOS / Linux 更新路径
- 强制更新、最低版本阻断
- 参考或翻译 `pot-desktop/` 实现

## 3. 背景与现状

| 能力 | 现状 |
|---|---|
| 发版 | tag `v*` → CI 构建 NSIS → 挂到 GitHub Release；`*-beta.*` 等标为 prerelease |
| 安装包 | `Shizi_x.y.z_x64-setup.exe` 等，用户从 Releases 页手动下载 |
| 设置 UI | 已有更新通道 + 自动检查开关；自动检查被 `DevOnly` 隐藏 |
| 配置持久化 | `updateChannel` / `autoCheckUpdate` 仅前端 localStorage 侧设置状态，**未**进入 `AppConfig` / `projectToAppConfig` |
| 打开链接 | `open_url` 仅允许 `https://`，Windows 用 `cmd /C start` |
| 依赖 | 后端已有 `reqwest`（rustls-tls + json） |

## 4. 方案对比与定稿

| 方案 | 内容 | 结论 |
|---|---|---|
| A 后端 GitHub API 检查 + 跳转下载 | Rust command 拉 Releases，前端提示并 `open_url` | **已选** |
| B 前端直连 GitHub API | WebView `fetch` | 否：CORS/限流/与配置层脱节 |
| C `tauri-plugin-updater` 仅检查 | 官方插件 + 静态 JSON | 否：本轮只需跳转，签名与 CI 过重 |

用户体验定稿：

1. **检查后跳转**，不应用内安装  
2. **触发**：启动自动检查（可关）+ 设置页手动检查  
3. **通道**：stable 仅正式版；beta 含 prerelease，取更高 semver  

## 5. 架构

```
┌─────────────────┐     invoke check_for_update      ┌──────────────────────┐
│ 设置页 / 启动钩子 │ ──────────────────────────────► │ ui/update.rs (command) │
└────────┬────────┘                                   └──────────┬───────────┘
         │ open_url(releaseUrl)                                   │
         ▼                                                        ▼
┌─────────────────┐                                   ┌──────────────────────┐
│ 系统默认浏览器   │                                   │ core/update/         │
│ Release 下载页   │                                   │  · GitHub client     │
└─────────────────┘                                   │  · channel filter    │
                                                      │  · semver compare    │
                                                      └──────────┬───────────┘
                                                                 │ HTTPS
                                                                 ▼
                                                      api.github.com/repos/
                                                      XuDaojie/shizi-translator/releases
```

分层约束：

- **核心逻辑**（解析 tag、通道过滤、版本比较）放 `src-tauri/src/core/update/`，可纯函数单测，不依赖 Tauri window
- **Command / 对话框 / 启动调度**放 `src-tauri/src/ui/` 与 `src-tauri/src/app/`（或等价现有装配点）
- 前端只做触发、loading、toast/对话框展示与「前往下载」调用 `open_url`；不实现版本比较

## 6. 配置模型

### 6.1 `AppConfig` 新增字段

与后端 `#[serde(rename_all = "camelCase")]` 对齐：

| 字段 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `updateChannel` | `"stable"` \| `"beta"` | `"stable"` | 更新通道 |
| `autoCheckUpdate` | `bool` | `true` | 启动时是否自动检查 |

Rust 侧：`#[serde(default = "...")]`，非法/缺失值在 `normalized`（或等价路径）归一为默认。

### 6.2 前后端同步

- `frontend/src/types/config.ts` 的 `AppConfig` 同步两字段  
- `projectToAppConfig`：从 `state.general.updateChannel` / `autoCheckUpdate` 写入  
- `mergeBackendInto*` / `syncFromBackend`：后端非空配置时覆盖前端同名字段（与现有 general 字段合并策略一致；若 general 无独立 merge 函数则在现有映射处补齐）  
- 存量用户 `config.json` 无字段：反序列化默认 → 稳定通道 + 开启自动检查  

### 6.3 仓库常量

GitHub 仓库**写死**为产品常量，不进用户配置：

- Owner / repo：`XuDaojie/shizi-translator`  
- API：`GET https://api.github.com/repos/XuDaojie/shizi-translator/releases?per_page=30`  
- 下载落地页：优先使用 release 的 `html_url`；兜底 `https://github.com/XuDaojie/shizi-translator/releases`  

请求头：

- `User-Agent: shizi/<version>`（或固定 `shizi` + version 查询参数外字段）— 满足 GitHub API 对 UA 的要求  
- `Accept: application/vnd.github+json`  
- **不**携带用户 GitHub token（公开仓库匿名访问即可）  

超时：建议 10–15s，失败映射为可展示错误，不阻塞主流程。

## 7. 版本与通道规则

### 7.1 当前版本

- 来源：编译期 `env!("CARGO_PKG_VERSION")`（与 `tauri.conf.json` / `Cargo.toml` 由发版脚本保持一致）  
- 前端展示可复用同一后端返回的 `currentVersion`，避免前后端各读一处不一致  

### 7.2 Tag 解析

- Release `tag_name` 允许 `v0.7.0` / `0.7.0` / `v0.7.0-beta.5`  
- 去掉可选前缀 `v` 后按 **semver 2.0** 解析（含 pre-release）  
- 无法解析的 tag：**跳过**该 release，不导致整次检查失败  
- Draft release：忽略（GitHub API 列表默认不含 draft；若字段存在则跳过）  

### 7.3 通道过滤

| 通道 | 候选集 |
|---|---|
| `stable` | `prerelease == false` 且无 pre-release 版本段的正式版 |
| `beta` | 全部非 draft 可解析 release（含正式版与 beta） |

在候选集中取 **semver 最高** 的一条作为 `latest`。  
若候选集为空：视为 `up_to_date` 或专用友好 message（推荐：`up_to_date`，message 说明无可用发布；实现计划二选一写死）。

### 7.4 是否有更新

- 若 `latest > current`（semver 序）→ `update_available`  
- 若 `latest <= current` → `up_to_date`  
- 相等含「当前已是通道内最新」  

依赖：Rust `semver` crate（实现阶段加入 `Cargo.toml`）。

## 8. Command 契约

### 8.1 `check_for_update`

```text
输入（可选）：
  channel?: "stable" | "beta"
    — 缺省则读当前 AppConfig.updateChannel

输出 CheckUpdateResult：
  status: "up_to_date" | "update_available" | "error"
  currentVersion: string
  latestVersion: string | null
  releaseName: string | null
  releaseUrl: string | null   // https only，供 open_url
  isPrerelease: boolean | null
  message: string | null      // 错误或补充说明（用户可读，中文或 i18n key 由实现计划定）
```

约定：

- `status == "error"` 时 `message` 必填；网络/HTTP 403/429/5xx 映射为简短中文（或 i18n）  
- 不在 command 内直接 `open_url`；由前端/启动流程在用户确认后调用  
- 启动自动检查与手动检查**共用**同一 command  

### 8.2 可选：`get_app_version`

若前端无其他方式拿版本不方便，可返回 `currentVersion` 字符串；也可并入 `check_for_update` 的 `currentVersion` 并在设置页静态展示时单独 command。实现计划二选一，优先最少 API。

## 9. 交互

### 9.1 设置页

「更新」`SettingGroup`：

1. **更新通道** — 已有 `SettingSelect`，绑定 `state.general.updateChannel`，保存进 `AppConfig`  
2. **自动检查更新** — 去掉 `DevOnly` 与 `status="wip"`，绑定 `autoCheckUpdate`  
3. **当前版本** — 展示 `currentVersion`（只读文案）  
4. **检查更新** — 按钮；检查中 disabled + loading  

手动检查反馈：

| 结果 | UI |
|---|---|
| `up_to_date` | toast：已是最新（含当前版本） |
| `update_available` | 对话框：最新版本、是否预发布、简短说明；主按钮「前往下载」、次按钮「稍后」 |
| `error` | toast：检查失败 + `message` |

「前往下载」：`invokeOpenUrl(releaseUrl)`；失败 toast 复用 `settings.toast.openUrlFailed`。

### 9.2 启动自动检查

- 时机：应用 setup 完成、配置可读之后，**异步**发起（不阻塞托盘/窗口）  
- 条件：`autoCheckUpdate == true`  
- 仅 `update_available` 时提示用户  
- `error` / `up_to_date`：**只记日志**，不弹窗、不 toast  
- 对话框宿主（实现时选一条写死，优先稳）：  
  - 优先：若 `settings` 窗口存在且可见 → 前端事件通知设置页弹对话框  
  - 否则：后端 `tauri-plugin-dialog` 消息框，或向 `main` 发事件由翻译弹窗展示  
  - 禁止因检查更新强制 `show` 翻译窗打断用户（除非产品后续另定）  

推荐默认路径：**后端 dialog 插件确认**（应用已依赖 `tauri-plugin-dialog`），避免依赖某 WebView 是否已挂载；确认后后端直接 `open_url`。设置页手动路径仍走前端 toast/对话框以保持风格统一。

> 实现计划须在「启动提示」与「设置页提示」两条路径上写清最终选型，避免双实现冲突。

### 9.3 i18n

- 去掉 wip 后，release 包需展示更新组文案（已有多语言 key 可复用）  
- 新增按钮/对话框/toast key（如 `settings.button.checkUpdate`、`settings.update.*`）写入 `zh-CN` / `en-US` 及现有 locale 文件（与项目 i18n 规范一致）  
- 后端 dialog 文案：可用中文产品常量或走现有 interface language 解析；实现计划选定，避免半英半中  

## 10. 错误处理与日志

| 场景 | 行为 |
|---|---|
| 无网络 / DNS / 超时 | `status=error`，message 网络失败；启动路径仅 log |
| HTTP 403/429 | 提示稍后重试 / 限流；log 含 status code |
| 响应非 JSON / 结构异常 | `status=error`；log debug 摘要 |
| 全部 tag 不可解析 | 按 §7.3 空候选处理 |
| `open_url` 失败 | 不改变检查结果；toast/log |
| 检查过程 panic | 不允许；错误用 `Result` 收敛 |

日志：

- info：检查开始/结束、通道、当前版本、最新版本、status  
- warn：HTTP 非 2xx、打开 URL 失败  
- debug：候选 release 数量、过滤后数量  
- **不**记录完整响应体（避免噪声）；API Key 无关本功能  

## 11. 安全

- 仅请求固定 HTTPS GitHub API 主机  
- `releaseUrl` 打开前校验 `https://`（复用 `open_url` 约束）；可选进一步限制 host 为 `github.com`  
- 不执行下载文件、不执行安装脚本  
- 匿名 API，无 token 落盘  

## 12. 测试

### 12.1 后端单测（必须）

- tag 解析：`v1.2.3`、`1.2.3`、`v0.7.0-beta.5`、非法 tag  
- semver 比较：正式版 vs beta、beta.1 vs beta.2、跨次版本  
- 通道过滤：stable 忽略 prerelease；beta 可选中更高正式版  
- 从 mock release 列表选出 latest  
- `AppConfig` 缺省字段默认值  

HTTP 客户端：优先对「解析 + 过滤」测纯函数；集成请求可用 mock（不必 CI 打真 GitHub）。

### 12.2 前端

- `projectToAppConfig` / merge 含新字段（扩展现有 `config.test.ts` / `settings.test.ts`）  
- 可选：检查按钮状态机（loading）轻量测  

### 12.3 手动验收

1. 稳定通道 + 当前已是最新 → toast 已最新  
2. 人为降低本地版本号或使用更高 mock → 对话框 → 前往下载打开正确 Release  
3. Beta 通道能看到 prerelease（当其版本更高）  
4. 关闭自动检查后重启不弹窗  
5. 断网：手动检查 toast 失败；启动无骚扰  

## 13. CI / 发版

**本轮不修改 CI。**

依据：检查逻辑只读现有 Releases 元数据；用户下载的仍是当前 workflow 上传的 `*-setup.exe`。

后续若做应用内安装，再单独立项：签名密钥、updater 清单、release 产物扩展。规格中明确该依赖链，避免本轮范围蔓延。

## 14. 文档同步（实现收尾）

实现完成并测试通过后须更新：

- `README.md`：下载安装旁补充「应用内检查更新」说明  
- `docs/roadmap/progressive-development-plan.md`：标记检查更新完成；自动检查不再列为 wip  
- `AGENTS.md` / `CLAUDE.md`：配置字段、`check_for_update`、启动检查行为；从 wip 列表移除「自动检查更新」  
- 本 spec 状态改为「已实现」  

## 15. 风险与缓解

| 风险 | 缓解 |
|---|---|
| GitHub API 匿名限流 | 仅启动一次 + 用户手动；短超时；友好错误 |
| 国内访问 GitHub 不稳定 | 失败不阻断主功能；用户仍可浏览器手动打开 Releases |
| 版本号与 tag 不一致 | 发版脚本已对齐；比较前统一剥 `v` |
| 启动对话框打断用户 | 仅有更新才提示；可关 `autoCheckUpdate` |
| 双 UI 路径（设置 toast vs 系统 dialog）不一致 | 实现计划写死选型 |

## 16. 成功标准

1. 用户可在设置中切换 stable/beta、开关自动检查，配置重启后仍在  
2. 手动检查能正确报告最新/已最新/失败  
3. 有更新时可一键打开对应 GitHub Release 页  
4. 启动自动检查默认开启、可关闭，失败不打扰  
5. 相关纯函数单测通过；`cargo test` / 前端相关 test 通过  
6. **未**引入 updater 插件，**未**改 release CI  
)
