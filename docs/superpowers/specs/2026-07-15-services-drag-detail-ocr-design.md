# 服务列表拖拽修复 · 翻译服务详情对齐 · 文字识别页复刻

- 日期：2026-07-15
- 状态：已确认设计，待实现
- 策略：以 OpenDesign 高保真原型为视觉与信息架构源，按模块迁入 shizi；保留 i18n、真实 Key 校验、DevOnly、save_app_config；OCR 本轮仅 UI + 配置持久化，运行时仍固定 Windows.Media.Ocr

## 1. 背景与目标

设置页「服务」面板存在三类待办：

1. **翻译服务列表无法拖动重排**：已有 HTML5 DnD 与 `reorderService`，但在 WebView2 下几乎拖不动。
2. **翻译服务详情与原型不一致**：主路径尚可，缺少原型级 Header 外链、高级区折叠、卡片式提示词编辑器。
3. **文字识别 Tab 仍是占位**：硬编码单条 Windows + 禁用「添加」，与 OpenDesign 可管理 OCR 域差距大。

原型位置：

- `OpenDesignProjects/shizi/src/settings/panels/ServicesPanel.vue`
- `OpenDesignProjects/shizi/src/settings/components/SettingTextarea.vue`
- 设计计划参考：`OpenDesignProjects/shizi/plan-ocr-services.md`、`plan-prompt-editor-ui.md`（本 spec 在其上按产品决策收窄）

**Done means**

1. 服务列表可拖动重排，**列表外观与现在一致**（不新增手柄、不改行样式）。
2. 翻译服务详情信息架构与交互对齐原型（Header / 主路径 / 高级折叠 / 卡片提示词），真实校验与持久化不回退。
3. 文字识别 Tab 可添加/配置多模态视觉 OCR 实例并持久化；默认保留 Windows；**运行时截图 OCR 行为不变**。
4. 翻译 Tab 与现有已对接翻译链路零回归。

## 2. 范围

### 2.1 范围内

| 项 | 内容 |
|---|---|
| 拖拽 | 仅修不可拖；不改列表视觉 |
| 翻译详情 | 完整对齐原型详情 + 卡片式 `SettingTextarea` |
| OCR UI | 左列表 + 添加（仅多模态）+ 右详情 system / vision-llm |
| OCR 数据 | `ocrServices[]` 类型、seed、store CRUD、前后端 `config.json` 持久化 |
| i18n | 新增文案走 `t()`（至少 zh-CN / en-US，其余 locale 可回退或同步补） |

### 2.2 范围外（YAGNI）

- 专用 OCR 整轨（百度/腾讯/有道/Azure/自定义传统 OCR）及 picker 分组
- 截图运行时按 `ocrServices` 选引擎或调用视觉模型
- OCR 列表拖拽 / 搜索
- 本地 PaddleOCR、语种包安装管理 UI
- 翻译与 OCR 实例自动同步开关（允许「填入翻译侧已有 Key」作可选 P1，不挡 P0）
- 全局提示词库、弹窗全屏编辑器、语法高亮

## 3. 已锁定决策

| # | 决策 | 状态 |
|---|---|---|
| 1 | 实现策略：按模块移植原型 + 保留 shizi 真实能力（非整页盲拷） | ✅ |
| 2 | 拖拽：只修功能，不加 Grip、不改列表样式；整行可拖与原型一致 | ✅ |
| 3 | 翻译详情：完整对齐原型（含卡片提示词方案 B） | ✅ |
| 4 | OCR：UI + 配置持久化；运行时仍仅 Windows.Media.Ocr | ✅ |
| 5 | 添加 OCR：**不要**专用 OCR；仅系统 Windows + 已对接且具备多模态的 LLM 视觉渠道 | ✅ |
| 6 | `ocrServices` 与翻译 `services` 永不共享实例 id | ✅ |
| 7 | Windows OCR：不可删除；不可关闭（保证截图路径始终可用） | ✅ |
| 8 | 视觉 OCR 允许多实例；本轮开关互不影响运行时（仅存配置） | ✅ |

## 4. 问题 1：列表拖拽修复

### 4.1 根因

列表行 `draggable=true`，主内容区为嵌套原生 `<button>`。Chromium / WebView2 下，可交互子元素常会阻断父级 HTML5 拖拽，表现为「拖不动」。开关侧已有 `@mousedown.stop`，不是主因。

### 4.2 修复原则

- **零样式变更**：不增加拖拽手柄、不改 padding/gap/字体/徽章布局。
- **行为修复**：
  1. 将行内用于「选中实例」的 `<button>` 改为非 button 可点击容器（如 `div` + `role="button"` + Enter/Space，或等价不拦截 DnD 的结构），保留 `@click` 选中。
  2. 开关/徽章容器继续 `@click.stop` + `@mousedown.stop`。
  3. 保留 `dragstart` / `dragover` / `drop` / `dragend`、`setData('text/plain')`、`effectAllowed = 'move'`、before/after 指示线与 `reorderService`。
- 顺序变更后走现有 dirty / 保存路径写入 `services[]` 顺序（与今日配置同步行为一致）。

### 4.3 验收

- [ ] 在 WebView2 设置窗口中可拖动重排，指示线与落点正确
- [ ] 侧视对比：列表行视觉与改前一致（允许 DOM 标签从 button→div 的不可见差异）
- [ ] 刷新/重开后顺序仍在（保存后）

## 5. 问题 2：翻译服务详情对齐原型

### 5.1 信息架构

```
Header
  实例名 [重命名] [AI?]
  description
  [查看文档] [申请 API Key]   ← 有 meta 链接时
  [删除]                      ← 与危险区二选一或并存：对齐原型 header 删除 + 保留危险组亦可；优先原型 header 删除，危险区可保留「删除实例」与现状一致时二选一并在实现中统一为原型形态

主路径（非 microsoft 等特殊分支保持现有逻辑）
  协议 / Endpoint / API Key / 默认模型

高级 · 提示词与推理（默认折叠）
  摘要：使用默认 | 已自定义 · 反思开
  展开：
    思维链（DevOnly + 现有 wip 策略）
    系统提示词卡片
    翻译提示词卡片（变量 chips：{source_lang} {target_lang} {text}）
    反思卡片（header 内 Switch；关=折叠说明；开=编辑区）

（可选）备注 custom
缺 Key 警告
```

### 5.2 `SettingTextarea` 卡片化

移植 OpenDesign 卡片式组件能力，**向后兼容**现有简单用法（无 `title` 时仍可用作备注等）：

| 能力 | 行为 |
|---|---|
| 顶栏 | 标题、描述、默认/已改 pill、字数、重置 |
| dirty | 仅显式内容且 ≠ default 时；空字符串 = 走默认，**重置写入空串**而非灌入默认全文 |
| 空态预览 | 有 default、model 为空且未 focus → 展示默认截断预览 +「使用默认 · 点击编辑」 |
| 字数 | focus 或 dirty 或有内容时显示 |
| 变量 chips | `variables` prop；点击插入光标处 |
| 折叠 | `collapsed` + `collapsedHint`（反思关闭） |
| 字体 | 保留 mono |
| status | 支持传入 `wip`（DevOnly 场景）；默认提示词行可不挂 wip |

主路径字段仍用现有 `SettingRow` / `ApiKeyInput` / `ModelCombobox`；**不**把提示词抬回主路径。

### 5.3 ServiceMeta 增量

为翻译渠道补可选：

```ts
docsUrl?: string
apiKeyUrl?: string
```

有值才渲染外链按钮。链接用 `target="_blank"` + Tauri 允许的打开方式（若 WebView 限制外链，用现有 shell open 模式，与项目其它外链一致）。

### 5.4 保留 shizi 能力

- Key 校验：`invokeValidateServiceCredential`（非 mock 随机）
- 模型拉取：`invokeListServiceModels`
- i18n：`t(...)`
- 未对接渠道：`protocols.length === 0` 的开发中标记与 release 隐藏不变
- `DevOnly`：思维链 / 反思等现有策略不变

### 5.5 验收

- [ ] 高级区默认折叠；摘要正确
- [ ] 三提示词可编辑、重置、空=默认语义
- [ ] 变量 chip 可插入
- [ ] 反思关/开布局不碎
- [ ] Key 校验 / 模型拉取 / 保存仍可用
- [ ] 窄栏（约 360–420px）重置不挤编辑区

## 6. 问题 3：文字识别页

### 6.1 信息架构

```
Tab 文字识别 (ocrServices.length)
├─ 左列表
│   ├─ 实例行：icon / 名 / 副标题 / 徽章 / 开关（Windows 无开关或 disabled）
│   └─ [添加 OCR 服务] → Dialog：仅「多模态视觉」一组
└─ 右详情
    ├─ system：Windows 关于 + 能力三栏 + 底部状态条
    └─ vision-llm：缺 Key 警告 + 基础配置 + 高级识别提示词（默认折叠）
```

不实现 `cloud-ocr` 详情分支与专用渠道 meta。

### 6.2 数据模型

```ts
export type OcrDetailKind = 'system' | 'vision-llm'

export type BuiltinOcrServiceId =
  | 'windows-media-ocr'
  | 'openai-vision'
  | 'claude-vision'
  | 'gemini-vision'
  | 'zhipu-vl'
  | 'siliconflow-vision'
  | 'moonshot-vision'
  | 'openai-compatible-vision'

export type OcrServiceInstance = {
  id: string
  type: BuiltinOcrServiceId | (string & {})
  name: string
  enabled: boolean
  apiKey: string
  endpoint: string
  note: string
  keyStatus: 'idle' | 'validating' | 'valid' | 'invalid'
  preferredLang: string  // 预留；vision 可选，UI 可先不做语种或放高级
  model: string
  pulledModels: string[]
  ocrPrompt: string      // 空 = DEFAULT_OCR_PROMPT
}
```

`AppSettings.ocrServices: OcrServiceInstance[]`

**seed**：仅 `windows-media-ocr`，`enabled: true`。

### 6.3 视觉渠道名单（本轮）

规则：翻译侧 **已对接**（`protocols.length > 0`）且为 **具备多模态能力的 LLM**。

| OCR type | 对应翻译 type | 默认模型示例 | 协议映射（配置用） |
|---|---|---|---|
| `windows-media-ocr` | — | — | system |
| `openai-vision` | openai | gpt-4o | openai_chat |
| `claude-vision` | claude | claude 多模态默认 | claude_messages |
| `gemini-vision` | gemini | gemini 多模态默认 | openai_chat（现有 Gemini 兼容端点） |
| `zhipu-vl` | zhipu | glm-4v 类 | openai_chat |
| `siliconflow-vision` | siliconflow | 用户自选 VL | openai_chat |
| `moonshot-vision` | moonshot | 视觉默认 | openai_chat |
| `openai-compatible-vision` | custom | 用户自填 | openai_chat，needsEndpoint |

**不纳入**：DeepSeek（当前对接以文本 chat 为主）、microsoft Edge、所有 `protocols: []` 的 ML 渠道、全部专用 OCR。

若实现时某渠道多模态能力存疑，可从 picker 去掉并在 tokens 注释原因；名单以 tokens 表为唯一源。

### 6.4 持久化

后端 `AppConfig` 增加：

```rust
#[serde(default)]
pub ocr_services: Vec<OcrServiceInstanceConfig>,
```

前端 `AppConfig` / `projectToAppConfig` / `mergeBackendIntoServices` 对称增加 `ocrServices` 合并（按 id：后端核心字段覆盖，前端独有 UI 态如 `keyStatus`/`pulledModels` 保留策略对齐翻译侧）。

旧配置无字段：`#[serde(default)]` + load 时若空则 seed Windows。

**运行时**：`ocr_translation` / 截图链路本轮 **不读** `ocr_services` 选引擎。配置仅设置页管理。

### 6.5 Store API

| 方法 | 行为 |
|---|---|
| `addOcrService(type)` | 建实例；`windows-media-ocr` 已存在则拒绝 |
| `removeOcrService(id)` | Windows no-op |
| `renameOcrService` | Windows 不可重命名或 no-op |
| toggle enabled | Windows 强制保持 enabled；其它可自由开关 |
| load/save | 随 `syncFromBackend` / `save` |

### 6.6 UI 细节

- Tab 计数 = `ocrServices.length`；去掉硬编码 `1` 与开发中占位。
- 副标题：system → 系统自带/离线类文案；vision → `{model \|\| '—'} · 多模态`。
- Picker 描述强调：与翻译实例独立，需单独添加。
- 视觉详情：复用 `ApiKeyInput`、`ModelCombobox`、卡片 `SettingTextarea`（识别提示词单条，默认折叠）。
- Key 校验：优先按映射协议走 `invokeValidateServiceCredential`（与翻译同 endpoint/key 形态）；失败则 toast，不阻塞输入。
- 模型拉取：`hasModelApi` 的视觉渠道走现有 list models（endpoint+key）；mock 拉取不作为主路径。

### 6.7 验收（P0）

- [ ] Tab 计数正确；无专用 OCR 入口
- [ ] 默认 Windows；不可删不可关
- [ ] 可添加上表视觉渠道；详情可填 Key/模型/Endpoint/识别提示词
- [ ] 保存后重开数据仍在
- [ ] 翻译 Tab 与拖拽修复无回归
- [ ] 截图 OCR 仍走 Windows（手动或既有测试路径确认）

## 7. 文件改动清单（预计）

| 文件 | 改动 |
|---|---|
| `frontend/src/settings/panels/ServicesPanel.vue` | 拖拽行为修复；详情高级区；OCR 整页数据驱动 |
| `frontend/src/settings/components/SettingTextarea.vue` | 卡片式编辑器 |
| `frontend/src/settings/types.ts` | Ocr* 类型、AppSettings.ocrServices；ServiceMeta 外链字段 |
| `frontend/src/settings/tokens.ts` | BUILTIN_OCR_SERVICES（system+vision）、DEFAULT_OCR_PROMPT、docs/apiKey 链接 |
| `frontend/src/settings/stores/settings.ts` | OCR CRUD、seed、sync/save |
| `frontend/src/lib/config.ts` + `config.test.ts` | project / validate 含 ocrServices |
| `frontend/src/types/config.ts` | AppConfig.ocrServices |
| `frontend/src/i18n/locales/*` | OCR / 详情新文案 |
| `src-tauri/src/core/config/types.rs` | OcrServiceInstanceConfig + AppConfig 字段 + default/normalized 测试 |
| 相关 settings 单测 | store / config-io 覆盖 seed 与合并 |

## 8. 实现顺序

1. 拖拽行为修复 + 手工/开发验证  
2. `SettingTextarea` 卡片化 + 单测/类型  
3. 翻译详情：Header 外链 + 高级折叠 + 接新 Textarea  
4. OCR 类型 / tokens / 后端 AppConfig 字段  
5. store seed + 前后端同步  
6. ServicesPanel OCR 左列表 + picker（仅 vision）  
7. OCR 右详情 system + vision  
8. i18n、验收清单、文档回填（README 能力说明可一句带过「OCR 服务配置预留」）

## 9. 风险与边界

| 风险 | 处理 |
|---|---|
| 用户误以为启用视觉 OCR 会立刻改变截图引擎 | 详情/说明文案标明「配置预留；当前截图识别使用 Windows 系统 OCR」 |
| 外链在 Tauri WebView 打不开 | 使用项目既有 open URL 方式 |
| `SettingTextarea` 重置语义变更（清空 vs 灌默认） | 与原型一致：空=默认；翻译侧已有空=默认约定，兼容 |
| 配置体积增大 | OCR 实例数量小，可忽略 |

## 10. 规格自检

- [x] 无 TODO/待定占位阻塞实现  
- [x] 拖拽 / 详情 / OCR 三块边界清晰，互不矛盾  
- [x] 范围收窄已写死（无专用 OCR、无运行时多引擎）  
- [x] 视觉名单规则明确（已对接 + 多模态 LLM）  
- [x] 与现有 services 事实来源（config.json）一致  

## 11. 参考

- 原型：`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\src\settings\panels\ServicesPanel.vue`
- 原型 Textarea：`...\components\SettingTextarea.vue`
- 计划草稿：`...\plan-ocr-services.md`、`...\plan-prompt-editor-ui.md`（本 spec 覆盖并收窄）
- 现状实现：`frontend/src/settings/panels/ServicesPanel.vue`
