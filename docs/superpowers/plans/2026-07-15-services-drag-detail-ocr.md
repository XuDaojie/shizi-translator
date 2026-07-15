# 服务列表拖拽 · 翻译详情对齐 · 文字识别页 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 修复设置页服务列表 HTML5 拖拽；将翻译服务详情与卡片式 `SettingTextarea` 对齐 OpenDesign 原型；实现文字识别 Tab 的 `ocrServices` 配置 UI 与持久化（仅 system + 多模态视觉，运行时仍固定 Windows OCR）。

**架构：** 前端 `settings` 模块按「tokens 元数据 → store CRUD → project/merge 配置 IO → ServicesPanel UI」分层扩展；后端 `AppConfig` 仅新增 `ocrServices` 的序列化与 default/normalized，**不**改 `ocr_translation` 运行时选引擎。拖拽只改 DOM 可拖性（`button` → 非 button 容器），列表 CSS 类名与布局零改动。`SettingTextarea` 以 OpenDesign 卡片组件为源移植，无 `title` 时保持简单 textarea 兼容。

**技术栈：** Vue 3 + TypeScript + Vitest（前端）、Rust serde + cargo test（后端）、Tauri `open_url` / `save_app_config`、i18n `t()`

**规格来源：** `docs/superpowers/specs/2026-07-15-services-drag-detail-ocr-design.md`

**原型参考（只读，禁止整页盲拷）：**

- `C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\src\settings\panels\ServicesPanel.vue`
- `C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi\src\settings\components\SettingTextarea.vue`
- 同目录 `types.ts` / `tokens.ts` / `stores/settings.ts`（OCR 部分需按 spec 收窄：无 dedicated OCR、Windows 不可关、视觉开关互不互斥）

---

## 文件结构

| 文件 | 职责 |
|---|---|
| 修改 `frontend/src/settings/panels/ServicesPanel.vue` | 拖拽行为；翻译详情 Header/高级折叠；OCR 左列表+picker+右详情 |
| 修改 `frontend/src/settings/components/SettingTextarea.vue` | 卡片式编辑器（title/variables/collapsed/空=默认） |
| 创建 `frontend/src/settings/components/setting-textarea-logic.ts` | 可单测的 dirty/默认/预览纯函数 |
| 创建 `frontend/src/settings/components/setting-textarea-logic.test.ts` | SettingTextarea 语义单测 |
| 修改 `frontend/src/settings/types.ts` | `Ocr*` 类型、`AppSettings.ocrServices`、`ServiceMeta.docsUrl/apiKeyUrl` |
| 修改 `frontend/src/settings/tokens.ts` | `docsUrl/apiKeyUrl`；`BUILTIN_OCR_SERVICES`（system+vision）；`DEFAULT_OCR_PROMPT`；`OCR_PICKER_SERVICES` |
| 修改 `frontend/src/settings/stores/settings.ts` | OCR seed/CRUD；`mergeBackendIntoOcrServices`；sync/save/dirty 纳入 ocrServices |
| 修改 `frontend/src/settings/stores/settings.test.ts` | OCR store + merge 单测 |
| 修改 `frontend/src/types/config.ts` | `OcrServiceInstanceConfig` + `AppConfig.ocrServices` |
| 修改 `frontend/src/lib/config.ts` | `projectToAppConfig` 投影 ocrServices |
| 修改 `frontend/src/lib/config.test.ts` | 投影与 fixture 含 ocrServices |
| 修改 `frontend/src/lib/tauri.ts` | 无新 command（复用 `invokeOpenUrl` / 既有校验） |
| 修改 `frontend/src/i18n/locales/zh-CN.json`、`en-US.json` | 新文案键 |
| 修改 `src-tauri/src/core/config/types.rs` | `OcrServiceInstanceConfig`、`AppConfig.ocr_services`、default/normalized 测试 |
| 修改 `docs/superpowers/specs/2026-07-15-services-drag-detail-ocr-design.md` | 收尾回填验收状态 |
| 修改 `README.md`（一句） | OCR 服务配置预留说明 |

**刻意不改：** `src-tauri/src/core/ocr_translation.rs`、截图/OCR 运行时选引擎、专用 OCR meta。

---

## 任务 1：服务列表拖拽行为修复

**文件：**
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`（列表行约 359–437）

**背景：** 行容器 `draggable=true`，主内容区嵌套原生 `<button>`，WebView2 下子 button 阻断 HTML5 DnD。开关侧已有 `@mousedown.stop`，不是主因。

- [ ] **步骤 1：记录改前结构（便于侧视对比）**

打开 `ServicesPanel.vue` 翻译列表 `v-for` 行，确认当前结构为：

```text
div[draggable] > button(选中) + div(@click.stop 徽章/开关) + drop indicator
```

- [ ] **步骤 2：将选中区 button 改为非 button 可点击容器**

把选中用的 `<button type="button" ... @click="onServiceSelect">` 改为：

```vue
<div
  role="button"
  tabindex="0"
  class="flex flex-1 items-start gap-2.5 text-left min-w-0 self-center cursor-pointer"
  @click="onServiceSelect(inst.id)"
  @keydown.enter.prevent="onServiceSelect(inst.id)"
  @keydown.space.prevent="onServiceSelect(inst.id)"
>
  <!-- 原 icon / name / model 内容原样保留，class 不改 -->
</div>
```

**禁止：** 新增 Grip 手柄、改 `px/py/gap`、改徽章布局、改开关侧结构。

- [ ] **步骤 3：确认 DnD 与 reorder 路径未动**

保留：`onDragStart` / `onDragOver` / `onDrop` / `onDragEnd`、`setData('text/plain')`、`effectAllowed = 'move'`、before/after 指示线、`settings.reorderService(...)`。开关容器继续 `@click.stop` + `@mousedown.stop`。

- [ ] **步骤 4：类型检查**

运行：

```powershell
npm run typecheck
```

预期：PASS（无新增错误）。

- [ ] **步骤 5：手工验收清单（开发时在 WebView2 勾）**

- 拖动整行可重排，指示线 before/after 正确
- 点击行仍选中实例；开关仍可点且不误拖
- 侧视：列表行视觉与改前一致
- 保存后刷新顺序仍在

- [ ] **步骤 6：Commit**

```powershell
git add frontend/src/settings/panels/ServicesPanel.vue
git commit -m "fix(settings): 修复服务列表 HTML5 拖拽被 button 阻断"
```

---

## 任务 2：SettingTextarea 卡片语义纯函数 + 单测（TDD）

**文件：**
- 创建：`frontend/src/settings/components/setting-textarea-logic.ts`
- 创建：`frontend/src/settings/components/setting-textarea-logic.test.ts`

**规格语义（与原型一致）：**

- dirty：仅 `showReset` 且有 `defaultValue` 且 **非空** 且 `modelValue !== defaultValue`
- 空字符串 = 走默认，**不算 dirty**
- 重置写入 `''`，不是灌入 default 全文
- 空态预览：有 default、model 为空、未 focus、未 collapsed

- [ ] **步骤 1：编写失败的测试**

创建 `setting-textarea-logic.test.ts`：

```ts
import { describe, expect, it } from 'vitest'
import {
  isPromptDirty,
  isPromptDefault,
  shouldShowDefaultPreview,
  shouldShowCharCount,
} from './setting-textarea-logic'

describe('setting-textarea-logic', () => {
  it('空串不算 dirty；显式改过才 dirty', () => {
    expect(isPromptDirty({ modelValue: '', defaultValue: 'DEF', showReset: true })).toBe(false)
    expect(isPromptDirty({ modelValue: 'DEF', defaultValue: 'DEF', showReset: true })).toBe(false)
    expect(isPromptDirty({ modelValue: '自定义', defaultValue: 'DEF', showReset: true })).toBe(true)
  })

  it('空或等于默认 → isDefault', () => {
    expect(isPromptDefault({ modelValue: '', defaultValue: 'DEF' })).toBe(true)
    expect(isPromptDefault({ modelValue: 'DEF', defaultValue: 'DEF' })).toBe(true)
    expect(isPromptDefault({ modelValue: 'x', defaultValue: 'DEF' })).toBe(false)
  })

  it('空态预览：空 model + 有 default + 未 focus + 未 collapsed', () => {
    expect(
      shouldShowDefaultPreview({
        modelValue: '',
        defaultValue: 'DEF',
        focused: false,
        collapsed: false,
      }),
    ).toBe(true)
    expect(
      shouldShowDefaultPreview({
        modelValue: '',
        defaultValue: 'DEF',
        focused: true,
        collapsed: false,
      }),
    ).toBe(false)
  })

  it('字数：focus 或 dirty 或有内容时显示', () => {
    expect(shouldShowCharCount({ collapsed: true, focused: true, dirty: true, charCount: 1 })).toBe(false)
    expect(shouldShowCharCount({ collapsed: false, focused: true, dirty: false, charCount: 0 })).toBe(true)
    expect(shouldShowCharCount({ collapsed: false, focused: false, dirty: true, charCount: 0 })).toBe(true)
    expect(shouldShowCharCount({ collapsed: false, focused: false, dirty: false, charCount: 3 })).toBe(true)
    expect(shouldShowCharCount({ collapsed: false, focused: false, dirty: false, charCount: 0 })).toBe(false)
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

```powershell
npx vitest run frontend/src/settings/components/setting-textarea-logic.test.ts
```

预期：FAIL（模块不存在）。

- [ ] **步骤 3：最小实现**

创建 `setting-textarea-logic.ts`：

```ts
export function isPromptDirty(opts: {
  modelValue: string
  defaultValue?: string
  showReset: boolean
}): boolean {
  if (!opts.showReset || opts.defaultValue === undefined) return false
  if (!opts.modelValue.trim()) return false
  return opts.modelValue !== opts.defaultValue
}

export function isPromptDefault(opts: {
  modelValue: string
  defaultValue?: string
}): boolean {
  if (opts.defaultValue === undefined) return false
  return !opts.modelValue.trim() || opts.modelValue === opts.defaultValue
}

export function shouldShowDefaultPreview(opts: {
  modelValue: string
  defaultValue?: string
  focused: boolean
  collapsed: boolean
}): boolean {
  return (
    !opts.collapsed &&
    !opts.modelValue.trim() &&
    !!opts.defaultValue?.trim() &&
    !opts.focused
  )
}

export function shouldShowCharCount(opts: {
  collapsed: boolean
  focused: boolean
  dirty: boolean
  charCount: number
}): boolean {
  if (opts.collapsed) return false
  return opts.focused || opts.dirty || opts.charCount > 0
}

/** 重置语义：写空串以走默认。 */
export function resetPromptValue(): string {
  return ''
}
```

- [ ] **步骤 4：运行测试验证通过**

```powershell
npx vitest run frontend/src/settings/components/setting-textarea-logic.test.ts
```

预期：PASS。

- [ ] **步骤 5：Commit**

```powershell
git add frontend/src/settings/components/setting-textarea-logic.ts frontend/src/settings/components/setting-textarea-logic.test.ts
git commit -m "test(settings): 添加 SettingTextarea 空=默认语义纯函数与单测"
```

---

## 任务 3：移植卡片式 SettingTextarea 组件

**文件：**
- 修改：`frontend/src/settings/components/SettingTextarea.vue`
- 参考：OpenDesign `SettingTextarea.vue`（完整卡片 UI）

- [ ] **步骤 1：扩展 Props 并接入纯函数**

在 `SettingTextarea.vue` 中：

```ts
import { computed, nextTick, ref } from 'vue'
import { Badge } from '@/components/ui/badge'
import {
  isPromptDirty,
  isPromptDefault,
  resetPromptValue,
  shouldShowCharCount,
  shouldShowDefaultPreview,
} from './setting-textarea-logic'
import { t } from '@/i18n'

interface Props {
  modelValue: string
  title?: string
  description?: string
  status?: 'wip' | 'planned'
  placeholder?: string
  defaultValue?: string
  variables?: string[]
  minRows?: number
  maxRows?: number
  disabled?: boolean
  showReset?: boolean
  collapsed?: boolean
  collapsedHint?: string
  className?: string
}

// defaults: minRows=3, maxRows=8, variables=[]
// isDirty / isDefault / showDefaultPreview / showCharCount 调用纯函数
// onReset: emit('update:modelValue', resetPromptValue())
// insertVariable: 光标处插入（无 ref 则追加末尾）
// 无 title 且无 header-end slot 时：仍渲染简洁编辑区（备注等兼容）
```

模板要点（对齐原型）：

1. 外层 `rounded-lg border` 卡片（**有 title 或卡片能力启用时**）
2. 顶栏：title、status Badge、已改/默认 pill、variables chips、字数、重置、`#header-end` slot
3. `collapsed` → 只显示 `collapsedHint`
4. `showDefaultPreview` → 「使用默认 · 点击编辑」+ default 截断预览
5. 否则 mono textarea
6. **兼容路径：** 调用方只传 `modelValue/defaultValue`、不传 `title` 时，仍可用（备注等）；顶栏可按 `title || isDirty || showCharCount || $slots['header-end']` 决定

i18n 文案键（任务 9 会落文件；此处先用 `t()`，键名见任务 9）：

- `settings.prompt.edited` / `settings.prompt.default` / `settings.prompt.useDefaultHint` / `settings.prompt.reset` / `settings.prompt.charCount` / `settings.prompt.insertVariable` / `settings.prompt.collapsed`

开发阶段若键缺失会显示 key 本身，任务 9 一并补齐。

- [ ] **步骤 2：typecheck**

```powershell
npm run typecheck
```

预期：PASS。

- [ ] **步骤 3：Commit**

```powershell
git add frontend/src/settings/components/SettingTextarea.vue
git commit -m "feat(settings): 卡片式 SettingTextarea 对齐 OpenDesign 语义"
```

---

## 任务 4：ServiceMeta 外链 + tokens docsUrl/apiKeyUrl

**文件：**
- 修改：`frontend/src/settings/types.ts`（`ServiceMeta` 增加可选字段）
- 修改：`frontend/src/settings/tokens.ts`（已对接渠道补链接）

- [ ] **步骤 1：类型增量**

在 `ServiceMeta` 末尾增加：

```ts
  /** 官方文档外链；有则详情 Header 显示「查看文档」。 */
  docsUrl?: string
  /** API Key 申请页；有则 Header / 缺 Key 警告显示「申请 API Key」。 */
  apiKeyUrl?: string
```

- [ ] **步骤 2：为已对接 LLM/ML 渠道填链接**

参考 OpenDesign tokens（仅填 shizi 已有渠道）。示例：

| id | docsUrl | apiKeyUrl |
|---|---|---|
| openai | https://developers.openai.com/api/docs | https://platform.openai.com/api-keys |
| deepseek | https://platform.deepseek.com/api-docs | https://platform.deepseek.com/api_keys |
| claude | https://docs.anthropic.com | https://console.anthropic.com/settings/keys |
| gemini | https://ai.google.dev/docs | https://aistudio.google.com/apikey |
| zhipu | https://open.bigmodel.cn/dev/api | https://open.bigmodel.cn/usercenter/apikeys |
| moonshot | https://platform.moonshot.cn/docs | https://platform.moonshot.cn/console/api-keys |
| siliconflow | https://docs.siliconflow.cn | https://cloud.siliconflow.cn/account/ak |
| custom | https://developers.openai.com/api/docs | （可省略 apiKeyUrl） |
| microsoft | 可省略或填 Edge/MS 文档 | 无 Key 则不填 apiKeyUrl |

`protocols: []` 的渠道可不填（release 隐藏）。

- [ ] **步骤 3：Commit**

```powershell
git add frontend/src/settings/types.ts frontend/src/settings/tokens.ts
git commit -m "feat(settings): ServiceMeta 增加文档与申请 Key 外链字段"
```

---

## 任务 5：翻译服务详情对齐原型

**文件：**
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`

- [ ] **步骤 1：增加 advanced 状态与摘要**

```ts
import { BookOpen, ExternalLink, KeyRound, ChevronDown } from '@lucide/vue'
import { invokeOpenUrl } from '@/lib/tauri'

const advancedOpen = ref(false)

const advancedSummary = computed(() => {
  const inst = activeInstance.value
  if (!inst) return ''
  const custom =
    !!inst.systemPrompt.trim() ||
    !!inst.translationPrompt.trim() ||
    !!inst.reflectionPrompt.trim()
  const parts: string[] = []
  parts.push(custom ? t('settings.prompt.summaryCustom') : t('settings.prompt.summaryDefault'))
  if (inst.reflectionEnabled) parts.push(t('settings.prompt.summaryReflectionOn'))
  return parts.join(' · ')
})

const openExternal = async (url: string): Promise<void> => {
  try {
    if (isTauriReady()) await invokeOpenUrl(url)
    else window.open(url, '_blank', 'noopener,noreferrer')
  } catch (err) {
    toast.error(t('settings.toast.openUrlFailed'), String(err))
  }
}

watch(activeInstanceId, () => {
  advancedOpen.value = false
  editingName.value = false
})
```

需 `import { isTauriReady } from '@/lib/tauri'`（若尚未导入）。

- [ ] **步骤 2：Header 外链按钮**

在翻译详情 header 的 description 之后（约现有 `activeService.description` 段落后）插入：

```vue
<div
  v-if="activeService.docsUrl || activeService.apiKeyUrl"
  class="mt-2 flex flex-wrap items-center gap-1.5"
>
  <button
    v-if="activeService.docsUrl"
    type="button"
    class="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
    @click="openExternal(activeService.docsUrl!)"
  >
    <BookOpen class="h-3 w-3" />
    {{ t('settings.button.viewDocs') }}
    <ExternalLink class="h-2.5 w-2.5 opacity-60" />
  </button>
  <button
    v-if="activeService.apiKeyUrl"
    type="button"
    class="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
    @click="openExternal(activeService.apiKeyUrl!)"
  >
    <KeyRound class="h-3 w-3" />
    {{ t('settings.button.applyApiKey') }}
    <ExternalLink class="h-2.5 w-2.5 opacity-60" />
  </button>
</div>
```

**注意：** 使用 `button` + `invokeOpenUrl`，不要依赖 WebView 内 `<a target=_blank>`（Tauri 外链不可靠）。URL 必须 `https://`。

- [ ] **步骤 3：主路径保持；高级区折叠**

保留非 microsoft 分支：协议 / Endpoint / API Key / 模型。

将「思维链 + 三提示词 + custom 备注」从独立 `SettingGroup` 迁入折叠块：

```vue
<div
  v-if="activeService.category === 'llm' || activeService.id === 'custom'"
  class="rounded-lg border border-border"
>
  <button
    type="button"
    class="flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left transition-colors hover:bg-accent/30"
    :aria-expanded="advancedOpen"
    @click="advancedOpen = !advancedOpen"
  >
    <span class="flex min-w-0 items-center gap-2">
      <ChevronDown
        :class="[
          'h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform',
          advancedOpen ? 'rotate-0' : '-rotate-90',
        ]"
      />
      <span class="text-xs font-medium text-foreground">{{ t('settings.group.advancedPrompts') }}</span>
      <span class="truncate text-[11px] text-muted-foreground">{{ advancedSummary }}</span>
    </span>
  </button>
  <div v-if="advancedOpen" class="space-y-3 border-t border-border px-3 py-3">
    <DevOnly>
      <!-- 思维链 SettingRow + SettingSelect，status=wip，仅 llm -->
    </DevOnly>

    <SettingTextarea
      v-if="activeService.category === 'llm'"
      :title="t('settings.field.systemPrompt')"
      :description="t('settings.description.systemPrompt')"
      :model-value="activeInstance.systemPrompt"
      :default-value="DEFAULT_PROMPTS.system"
      @update:model-value="(v) => (activeInstance!.systemPrompt = v)"
    />
    <SettingTextarea
      v-if="activeService.category === 'llm'"
      :title="t('settings.field.translationPrompt')"
      :description="t('settings.description.translationPrompt')"
      :variables="['{source_lang}', '{target_lang}', '{text}']"
      :model-value="activeInstance.translationPrompt"
      :default-value="DEFAULT_PROMPTS.translation"
      @update:model-value="(v) => (activeInstance!.translationPrompt = v)"
    />
    <DevOnly>
      <SettingTextarea
        v-if="activeService.category === 'llm'"
        :title="t('settings.field.reflectionPrompt')"
        :description="t('settings.description.reflectionPrompt')"
        status="wip"
        :model-value="activeInstance.reflectionPrompt"
        :default-value="DEFAULT_PROMPTS.reflection"
        :collapsed="!activeInstance.reflectionEnabled"
        :collapsed-hint="t('settings.prompt.reflectionCollapsed')"
        @update:model-value="(v) => (activeInstance!.reflectionPrompt = v)"
      >
        <template #header-end>
          <SettingSwitch
            :model-value="activeInstance.reflectionEnabled"
            @update:model-value="(v) => (activeInstance!.reflectionEnabled = v)"
          />
        </template>
      </SettingTextarea>
    </DevOnly>

    <!-- custom 备注 SettingRow + SettingInput -->
  </div>
</div>
```

- 危险区「删除实例」保留
- 缺 Key 警告可链到 `applyApiKey`（有 `apiKeyUrl` 时 `openExternal`）
- Key 校验 / 模型拉取路径不变

- [ ] **步骤 4：typecheck**

```powershell
npm run typecheck
```

- [ ] **步骤 5：Commit**

```powershell
git add frontend/src/settings/panels/ServicesPanel.vue
git commit -m "feat(settings): 翻译服务详情对齐原型高级折叠与外链"
```

---

## 任务 6：后端 AppConfig.ocr_services（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [ ] **步骤 1：编写失败的 Rust 测试**

在 `types.rs` 的 `mod tests` 末尾增加：

```rust
#[test]
fn ocr_services_default_empty_and_deserializes_missing_as_empty() {
    let config = AppConfig::default();
    assert!(config.ocr_services.is_empty());

    let json = r#"{"targetLang":"zh-CN","services":[]}"#;
    let parsed: AppConfig = serde_json::from_str(json).expect("parse");
    assert!(parsed.ocr_services.is_empty());
}

#[test]
fn ocr_services_roundtrip_camel_case() {
    let mut config = AppConfig::default();
    config.ocr_services = vec![OcrServiceInstanceConfig {
        id: "ocr-win".into(),
        service_type: "windows-media-ocr".into(),
        name: "Windows 媒体 OCR".into(),
        enabled: true,
        api_key: None,
        endpoint: String::new(),
        model: String::new(),
        preferred_lang: String::new(),
        ocr_prompt: String::new(),
    }];
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("ocrServices"));
    assert!(json.contains("serviceType"));
    assert!(json.contains("preferredLang"));
    assert!(json.contains("ocrPrompt"));
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.ocr_services.len(), 1);
    assert_eq!(back.ocr_services[0].service_type, "windows-media-ocr");
    assert!(back.ocr_services[0].enabled);
}

#[test]
fn normalized_trims_ocr_service_fields() {
    let mut config = AppConfig::default();
    config.ocr_services = vec![OcrServiceInstanceConfig {
        id: "  ocr-1  ".into(),
        service_type: "openai-vision".into(),
        name: "  V  ".into(),
        enabled: true,
        api_key: Some("  sk  ".into()),
        endpoint: "  https://api.openai.com/v1  ".into(),
        model: "  gpt-4o  ".into(),
        preferred_lang: "  en  ".into(),
        ocr_prompt: "  hello  ".into(),
    }];
    let n = config.normalized();
    assert_eq!(n.ocr_services[0].id, "ocr-1");
    assert_eq!(n.ocr_services[0].name, "V");
    assert_eq!(n.ocr_services[0].api_key.as_deref(), Some("sk"));
    assert_eq!(n.ocr_services[0].endpoint, "https://api.openai.com/v1");
    assert_eq!(n.ocr_services[0].model, "gpt-4o");
    assert_eq!(n.ocr_services[0].preferred_lang, "en");
    assert_eq!(n.ocr_services[0].ocr_prompt, "hello");
}
```

- [ ] **步骤 2：运行测试验证失败**

```powershell
cd src-tauri; cargo test ocr_services -- --nocapture
```

预期：FAIL（类型不存在）。

- [ ] **步骤 3：最小实现**

在 `ServiceInstanceConfig` 旁增加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrServiceInstanceConfig {
    pub id: String,
    pub service_type: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub preferred_lang: String,
    #[serde(default)]
    pub ocr_prompt: String,
}

impl OcrServiceInstanceConfig {
    pub fn normalized(mut self) -> Self {
        self.id = self.id.trim().to_string();
        self.service_type = self.service_type.trim().to_string();
        self.name = self.name.trim().to_string();
        self.api_key = self.api_key.and_then(non_empty_string);
        self.endpoint = self.endpoint.trim().to_string();
        self.model = self.model.trim().to_string();
        self.preferred_lang = self.preferred_lang.trim().to_string();
        self.ocr_prompt = self.ocr_prompt.trim().to_string();
        self
    }
}
```

`AppConfig` 增加：

```rust
#[serde(default)]
pub ocr_services: Vec<OcrServiceInstanceConfig>,
```

`AppConfig::default` 初始化 `ocr_services: vec![]`（seed Windows 由前端负责，与「后端空 services 推前端」对称：后端可空，前端 sync 时补 seed）。

`normalized`：

```rust
self.ocr_services = self
    .ocr_services
    .into_iter()
    .map(|s| s.normalized())
    .collect();
```

**不要**在 `ocr_translation` 读取该字段。

- [ ] **步骤 4：运行测试验证通过**

```powershell
cd src-tauri; cargo test ocr_services -- --nocapture
```

预期：PASS。

- [ ] **步骤 5：Commit**

```powershell
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): AppConfig 增加 ocrServices 持久化字段"
```

---

## 任务 7：前端 OCR 类型、tokens、配置投影（TDD）

**文件：**
- 修改：`frontend/src/settings/types.ts`
- 修改：`frontend/src/settings/tokens.ts`
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`

- [ ] **步骤 1：编写失败的 config 测试**

在 `config.test.ts` 的 `makeState` 增加 `ocrServices: []`，并新增：

```ts
it('投影 ocrServices 到后端 camelCase 字段', () => {
  const state = makeState([])
  state.ocrServices = [
    {
      id: 'ocr-win',
      type: 'windows-media-ocr',
      name: 'Windows 媒体 OCR',
      enabled: true,
      apiKey: '',
      endpoint: '',
      note: '',
      keyStatus: 'idle',
      preferredLang: '',
      model: '',
      pulledModels: [],
      ocrPrompt: '',
    },
    {
      id: 'ocr-v',
      type: 'openai-vision',
      name: 'OpenAI 视觉',
      enabled: false,
      apiKey: 'sk-test',
      endpoint: 'https://api.openai.com/v1',
      note: 'n',
      keyStatus: 'valid',
      preferredLang: '',
      model: 'gpt-4o',
      pulledModels: ['gpt-4o'],
      ocrPrompt: '读图',
    },
  ]
  const config = projectToAppConfig(state)
  expect(config.ocrServices).toHaveLength(2)
  expect(config.ocrServices[0]).toMatchObject({
    id: 'ocr-win',
    serviceType: 'windows-media-ocr',
    enabled: true,
    apiKey: null,
  })
  expect(config.ocrServices[1]).toMatchObject({
    serviceType: 'openai-vision',
    apiKey: 'sk-test',
    model: 'gpt-4o',
    ocrPrompt: '读图',
  })
  // note / keyStatus / pulledModels 不进后端
  expect(config.ocrServices[1]).not.toHaveProperty('note')
  expect(config.ocrServices[1]).not.toHaveProperty('keyStatus')
  expect(config.ocrServices[1]).not.toHaveProperty('pulledModels')
})
```

同时修所有 `AppSettings` / `AppConfig` fixture 缺字段导致的编译错误。

- [ ] **步骤 2：运行验证失败**

```powershell
npx vitest run frontend/src/lib/config.test.ts
```

预期：类型或断言失败。

- [ ] **步骤 3：类型与 tokens**

`types.ts` 增加（与 spec §6.2 一致，收窄 id 列表）：

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

export type OcrServiceId = BuiltinOcrServiceId | (string & {})

export type OcrServiceMeta = {
  id: OcrServiceId
  name: string
  description: string
  detail?: string
  builtin: boolean
  keyRequired: boolean
  canDisable: boolean
  canDelete: boolean
  multiInstance?: boolean
  protocol?: string
  /** 配置用协议 id，供 Key 校验 / 拉模型复用翻译 probe。 */
  protocolId?: 'openai_chat' | 'claude_messages'
  apiBaseUrl?: string
  docsUrl?: string
  apiKeyUrl?: string
  needsEndpoint?: boolean
  hasModelApi?: boolean
  defaultModel?: string
  models?: string[]
  iconifyId?: string
  detailKind: OcrDetailKind
  group: 'system' | 'vision'
}

export type OcrServiceInstance = {
  id: string
  type: OcrServiceId
  name: string
  enabled: boolean
  apiKey: string
  endpoint: string
  note: string
  keyStatus: 'idle' | 'validating' | 'valid' | 'invalid'
  preferredLang: string
  model: string
  pulledModels: string[]
  ocrPrompt: string
}

// AppSettings 增加：
ocrServices: OcrServiceInstance[]
```

`tokens.ts`：

```ts
export const DEFAULT_OCR_PROMPT = '提取图中全部文字，保持阅读顺序'

export const BUILTIN_OCR_SERVICES: OcrServiceMeta[] = [
  {
    id: 'windows-media-ocr',
    name: 'Windows 媒体 OCR',
    description: 'Windows 10+ 系统自带 OCR，无需 API Key。',
    detail: '配置预留；当前截图识别固定使用 Windows.Media.Ocr，与下方视觉实例启用状态无关。',
    builtin: true,
    keyRequired: false,
    canDisable: false, // spec：不可关
    canDelete: false,
    detailKind: 'system',
    group: 'system',
  },
  // vision only — 对应翻译侧已对接且多模态 LLM
  {
    id: 'openai-vision',
    name: 'OpenAI 视觉',
    description: 'GPT-4o 等多模态模型识图。与翻译实例独立，需单独添加。',
    builtin: true,
    keyRequired: true,
    canDisable: true,
    canDelete: true,
    multiInstance: true,
    hasModelApi: true,
    defaultModel: 'gpt-4o',
    models: ['gpt-4o', 'gpt-4o-mini'],
    protocolId: 'openai_chat',
    apiBaseUrl: 'https://api.openai.com/v1',
    detailKind: 'vision-llm',
    group: 'vision',
    iconifyId: 'simple-icons:openai',
    docsUrl: 'https://developers.openai.com/api/docs',
    apiKeyUrl: 'https://platform.openai.com/api-keys',
  },
  // claude-vision → protocolId: claude_messages, apiBaseUrl anthropic
  // gemini-vision → openai_chat + generativelanguage .../openai 端点（与翻译 gemini 一致）
  // zhipu-vl → openai_chat + open.bigmodel.cn
  // siliconflow-vision → openai_chat
  // moonshot-vision → openai_chat（若多模态存疑，可保留但在注释标明；以 tokens 为唯一源）
  // openai-compatible-vision → needsEndpoint, openai_chat
]

export const ocrServiceById = (id: OcrServiceId) =>
  BUILTIN_OCR_SERVICES.find((s) => s.id === id)

/** picker：仅 vision 组。 */
export const OCR_PICKER_SERVICES = BUILTIN_OCR_SERVICES.filter((s) => s.group === 'vision')
```

**禁止**把 baidu-ocr / dedicated 组拷进 shizi。

`types/config.ts`：

```ts
export interface OcrServiceInstanceConfig {
  id: string
  serviceType: string
  name: string
  enabled: boolean
  apiKey: string | null
  endpoint: string
  model: string
  preferredLang: string
  ocrPrompt: string
}

export interface AppConfig {
  // ...existing
  ocrServices: OcrServiceInstanceConfig[]
}
```

`projectToAppConfig`：

```ts
ocrServices: state.ocrServices.map((s) => ({
  id: s.id,
  serviceType: s.type,
  name: s.name,
  enabled: s.enabled,
  apiKey: s.apiKey.trim() || null,
  endpoint: s.endpoint.trim(),
  model: s.model.trim(),
  preferredLang: s.preferredLang.trim(),
  ocrPrompt: s.ocrPrompt.trim(),
})),
```

`validateConfig`：**不要**因 OCR 缺 Key 阻断保存（配置预留；仅翻译 enabled 服务校验）。

- [ ] **步骤 4：测试通过**

```powershell
npx vitest run frontend/src/lib/config.test.ts
npm run typecheck
```

- [ ] **步骤 5：Commit**

```powershell
git add frontend/src/settings/types.ts frontend/src/settings/tokens.ts frontend/src/types/config.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts
git commit -m "feat(settings): OCR 类型 tokens 与 config 投影"
```

---

## 任务 8：settings store OCR seed / CRUD / 合并同步（TDD）

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`

**行为契约（与 OpenDesign 不同处按 spec）：**

| 方法 | 行为 |
|---|---|
| seed | 仅 `windows-media-ocr`，`enabled: true` |
| `addOcrService(type)` | 建实例默认 `enabled: false`；Windows 已存在则返回已有 |
| `removeOcrService(id)` | Windows no-op；其它删除 |
| `setOcrEnabled(id, enabled)` | Windows 强制 `enabled=true`（忽略关闭）；视觉自由开关，**不**互斥 |
| `renameOcrService` | Windows no-op |
| merge | 按 id：后端核心字段覆盖；`keyStatus`/`pulledModels`/`note` 前端保留 |
| sync | 后端无 ocr 或空 → 保留/补 seed Windows；非空则 merge |
| dirty | `serializeForDirty` 含 ocrServices，ocr 的 keyStatus 归一 idle |

- [ ] **步骤 1：编写失败的 store 测试**

```ts
import { mergeBackendIntoOcrServices, useSettings } from './settings'
import { DEFAULT_OCR_PROMPT } from '../tokens' // 若 seed 用空 prompt 可不测常量

describe('ocrServices store', () => {
  it('默认 seed 仅 Windows 且启用', () => {
    const s = useSettings()
    expect(s.state.ocrServices).toHaveLength(1)
    expect(s.state.ocrServices[0].type).toBe('windows-media-ocr')
    expect(s.state.ocrServices[0].enabled).toBe(true)
  })

  it('添加视觉实例；不可添加重复 Windows', () => {
    const s = useSettings()
    const win = s.addOcrService('windows-media-ocr')
    expect(s.state.ocrServices.filter((x) => x.type === 'windows-media-ocr')).toHaveLength(1)
    expect(win.id).toBe(s.state.ocrServices[0].id)

    const v = s.addOcrService('openai-vision')
    expect(v.enabled).toBe(false)
    expect(v.ocrPrompt).toBe('')
    expect(s.state.ocrServices).toHaveLength(2)
  })

  it('Windows 不可删不可关；视觉可关可删且不互斥', () => {
    const s = useSettings()
    const winId = s.state.ocrServices[0].id
    s.removeOcrService(winId)
    expect(s.state.ocrServices).toHaveLength(1)
    s.setOcrEnabled(winId, false)
    expect(s.state.ocrServices[0].enabled).toBe(true)

    const a = s.addOcrService('openai-vision')
    const b = s.addOcrService('claude-vision')
    s.setOcrEnabled(a.id, true)
    s.setOcrEnabled(b.id, true)
    expect(s.state.ocrServices.find((x) => x.id === a.id)!.enabled).toBe(true)
    expect(s.state.ocrServices.find((x) => x.id === b.id)!.enabled).toBe(true)
    s.removeOcrService(a.id)
    expect(s.state.ocrServices.some((x) => x.id === a.id)).toBe(false)
  })

  it('mergeBackendIntoOcrServices 保留 keyStatus/pulledModels/note', () => {
    const local = [/* windows + vision with note/keyStatus/pulledModels */]
    const backend = [/* same ids, different apiKey/model */]
    const result = mergeBackendIntoOcrServices(local, backend)
    // assert 核心字段来自后端，UI 字段来自 local
  })
})
```

（测试内补全 `makeLocalOcr` / `makeBackendOcr` helper，风格对齐 `makeLocal` / `makeBackend`。）

- [ ] **步骤 2：运行失败**

```powershell
npx vitest run frontend/src/settings/stores/settings.test.ts -t ocrServices
```

- [ ] **步骤 3：实现 store**

要点：

```ts
const defaultOcrInstanceFor = (type: OcrServiceId, name: string, enabled = false): OcrServiceInstance => {
  const meta = ocrServiceById(type)
  return {
    id: newInstanceId(),
    type,
    name,
    enabled,
    apiKey: '',
    endpoint: meta?.apiBaseUrl ?? '',
    note: '',
    keyStatus: 'idle',
    preferredLang: '',
    model: meta?.defaultModel ?? '',
    pulledModels: [],
    ocrPrompt: '',
  }
}

const seedOcrInstances = (): OcrServiceInstance[] => {
  const win = BUILTIN_OCR_SERVICES.find((s) => s.id === 'windows-media-ocr')
  if (!win) return []
  return [defaultOcrInstanceFor(win.id, win.name, true)]
}

// buildDefaults / loadFromStorage 纳入 ocrServices
// loadFromStorage：无字段或空数组 → seedOcrInstances()；并保证至少有 Windows

export const mergeBackendIntoOcrServices = (
  local: OcrServiceInstance[],
  backend: OcrServiceInstanceConfig[],
): OcrServiceInstance[] => {
  if (!backend.length) {
    // 保证 Windows seed
    return local.length ? local : seedOcrInstances()
  }
  const localById = new Map(local.map((s) => [s.id, s]))
  return backend.map((b) => {
    const existing = localById.get(b.id)
    const base: OcrServiceInstance = existing ?? {
      id: b.id,
      type: b.serviceType as OcrServiceId,
      name: b.name,
      enabled: b.enabled,
      apiKey: b.apiKey ?? '',
      endpoint: b.endpoint,
      note: '',
      keyStatus: 'idle',
      preferredLang: b.preferredLang ?? '',
      model: b.model,
      pulledModels: [],
      ocrPrompt: b.ocrPrompt ?? '',
    }
    if (!existing) return { ...base, /* from backend fields */ }
    return {
      ...existing,
      name: b.name,
      enabled: b.serviceType === 'windows-media-ocr' ? true : b.enabled,
      apiKey: b.apiKey ?? '',
      endpoint: b.endpoint,
      model: b.model,
      preferredLang: b.preferredLang ?? '',
      ocrPrompt: b.ocrPrompt ?? '',
      type: b.serviceType as OcrServiceId,
    }
  })
}

// syncFromBackend 在 merge services 后：
// state.ocrServices = mergeBackendIntoOcrServices(state.ocrServices, backend.ocrServices ?? [])
// 若结果无 Windows，unshift seed Windows

// serializeForDirty:
// ocrServices: s.ocrServices.map((o) => ({ ...o, keyStatus: 'idle' }))

// API: addOcrService / removeOcrService / setOcrEnabled / renameOcrService / findOcrInstance
// getMergedOcrServices(): return BUILTIN_OCR_SERVICES
```

- [ ] **步骤 4：测试通过 + 修 sync 相关 fixture**

凡构造 `AppConfig` 的测试补 `ocrServices: []`。

```powershell
npx vitest run frontend/src/settings/stores/settings.test.ts
npm run typecheck
```

- [ ] **步骤 5：Commit**

```powershell
git add frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(settings): OCR 实例 seed CRUD 与后端合并同步"
```

---

## 任务 9：i18n 文案（zh-CN / en-US）

**文件：**
- 修改：`frontend/src/i18n/locales/zh-CN.json`
- 修改：`frontend/src/i18n/locales/en-US.json`

- [ ] **步骤 1：新增键（至少）**

| key | zh-CN 示例 |
|---|---|
| `settings.button.viewDocs` | 查看文档 |
| `settings.button.applyApiKey` | 申请 API Key |
| `settings.group.advancedPrompts` | 高级 · 提示词与推理 |
| `settings.prompt.edited` | 已改 |
| `settings.prompt.default` | 默认 |
| `settings.prompt.useDefaultHint` | 使用默认 · 点击编辑 |
| `settings.prompt.reset` | 重置为默认 |
| `settings.prompt.charCount` | {n} 字符 |
| `settings.prompt.insertVariable` | 插入 {v} |
| `settings.prompt.collapsed` | 已关闭 |
| `settings.prompt.reflectionCollapsed` | 关闭时跳过译后自检。 |
| `settings.prompt.summaryDefault` | 使用默认 |
| `settings.prompt.summaryCustom` | 已自定义 |
| `settings.prompt.summaryReflectionOn` | 反思开 |
| `settings.toast.openUrlFailed` | 打开链接失败 |
| `settings.description.addOcrService` | 仅多模态视觉渠道；与翻译实例独立，需单独配置。当前截图识别仍使用 Windows 系统 OCR。 |
| `settings.ocr.visionSubtitle` | {model} · 多模态 |
| `settings.ocr.systemSubtitle` | 系统自带 · 离线 |
| `settings.ocr.configReserved` | 配置预留；当前截图识别使用 Windows 系统 OCR |
| `settings.field.ocrPrompt` | 识别提示词 |
| `settings.description.ocrPrompt` | 指导视觉模型如何提取文字；空则使用默认。 |

同步 en-US 英文译文。其它 locale 可回退到 zh-CN/en-US 现有机制。

- [ ] **步骤 2：Commit**

```powershell
git add frontend/src/i18n/locales/zh-CN.json frontend/src/i18n/locales/en-US.json
git commit -m "feat(i18n): 服务详情与 OCR 配置相关文案"
```

---

## 任务 10：ServicesPanel OCR 左列表 + 添加 Dialog

**文件：**
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`

- [ ] **步骤 1：状态与选择**

```ts
import {
  DEFAULT_OCR_PROMPT,
  OCR_PICKER_SERVICES,
  ocrServiceById,
  serviceById,
} from '../tokens'
import type { OcrServiceId, OcrServiceInstance } from '../types'

const activeOcrInstanceId = ref(props.state.ocrServices[0]?.id ?? '')
const ocrPickerOpen = ref(false)

const activeOcrInstance = computed(() =>
  props.state.ocrServices.find((s) => s.id === activeOcrInstanceId.value),
)
const activeOcrService = computed(() =>
  activeOcrInstance.value ? ocrServiceById(activeOcrInstance.value.type) : undefined,
)

watch(
  () => props.state.ocrServices.map((s) => s.id).join(','),
  () => {
    if (!props.state.ocrServices.some((s) => s.id === activeOcrInstanceId.value)) {
      activeOcrInstanceId.value = props.state.ocrServices[0]?.id ?? ''
    }
  },
)

const ocrSubtitle = (inst: OcrServiceInstance): string => {
  const meta = ocrServiceById(inst.type)
  if (meta?.detailKind === 'system') return t('settings.ocr.systemSubtitle')
  return t('settings.ocr.visionSubtitle', { model: inst.model || '—' })
}

const onAddOcrService = (type: OcrServiceId): void => {
  const inst = settings.addOcrService(type)
  activeOcrInstanceId.value = inst.id
  ocrPickerOpen.value = false
}
```

- [ ] **步骤 2：替换 OCR Tab 占位**

- Tab 计数：`props.state.ocrServices.length`（去掉硬编码 `1`）
- 左列表：`v-for="inst in state.ocrServices"`
  - icon：`ScanText` 或 `ServiceIcon`（vision 可复用对应翻译 iconify；无映射则 ScanText）
  - 名 / 副标题 / 徽章（system 内置；vision 需 Key）
  - 开关：`meta.canDisable === false` 时不渲染或 disabled + 始终 on；其它 `@update` → `settings.setOcrEnabled`
- 底部「添加 OCR 服务」：启用 Dialog，`OCR_PICKER_SERVICES` 单组「多模态视觉」
- **不要** dedicated 分组、不要禁用按钮 + Lock 占位

- [ ] **步骤 3：typecheck**

```powershell
npm run typecheck
```

- [ ] **步骤 4：Commit**

```powershell
git add frontend/src/settings/panels/ServicesPanel.vue
git commit -m "feat(settings): 文字识别列表与视觉渠道添加"
```

---

## 任务 11：OCR 右详情 system + vision-llm

**文件：**
- 修改：`frontend/src/settings/panels/ServicesPanel.vue`

- [ ] **步骤 1：system 详情**

保留现有 Windows 关于 / 三栏能力 / 底部状态条结构，改为绑定 `activeOcrInstance` + `activeOcrService`：

- Header 名来自实例；不可重命名（不显示铅笔）或 rename no-op
- 增加醒目说明：`t('settings.ocr.configReserved')`
- 无删除按钮（`canDelete === false`）

- [ ] **步骤 2：vision-llm 详情**

```text
Header：名 [重命名] + description + docs/apiKey 外链 + [删除]
缺 Key 警告 + 申请链接
基础配置：ApiKeyInput / ModelCombobox / Endpoint（needsEndpoint 或始终可编）
高级折叠：SettingTextarea 识别提示词
  title=ocrPrompt, default=DEFAULT_OCR_PROMPT, 默认 collapsed 可用 advancedOcrOpen
底部：configReserved 提示条
```

Key 校验：

```ts
const probeOcrRequest = (inst: OcrServiceInstance) => {
  const meta = ocrServiceById(inst.type)
  return {
    protocol: meta?.protocolId ?? 'openai_chat',
    endpoint: inst.endpoint,
    apiKey: inst.apiKey.trim() || null,
  }
}
// invokeValidateServiceCredential(probeOcrRequest(inst))
// 拉模型：meta.hasModelApi → invokeListServiceModels
```

注意：OCR 的 `keyStatus` 可写在实例上（会进 localStorage），`serializeForDirty` 已归一。

- [ ] **步骤 3：切换 tab 时详情互不干扰**

`tab === 'ocr'` 显示 OCR 详情；`tab === 'translate'` 显示翻译详情。两边 empty state 保留。

- [ ] **步骤 4：typecheck + 前端单测**

```powershell
npm run typecheck
npm run test
```

- [ ] **步骤 5：Commit**

```powershell
git add frontend/src/settings/panels/ServicesPanel.vue
git commit -m "feat(settings): 文字识别 system/vision 详情页"
```

---

## 任务 12：端到端验收与文档回填

**文件：**
- 修改：`docs/superpowers/specs/2026-07-15-services-drag-detail-ocr-design.md`（验收复选框）
- 修改：`docs/superpowers/plans/2026-07-15-services-drag-detail-ocr.md`（本计划任务勾选）
- 修改：`README.md`（一句：设置页可配置 OCR 服务实例，当前截图识别仍为 Windows OCR）

- [ ] **步骤 1：自动化回归**

```powershell
npm run test
npm run typecheck
cd src-tauri; cargo test ocr_services; cargo test
```

预期：全部 PASS。

- [ ] **步骤 2：手工验收（spec §4.3 / §5.5 / §6.7）**

- [ ] WebView2 服务列表可拖；视觉无回归
- [ ] 高级区默认折叠；卡片提示词 dirty/重置/变量 chip/反思折叠
- [ ] Key 校验 / 拉模型 / 保存仍可用
- [ ] OCR Tab 计数正确；无专用 OCR；默认 Windows 不可删不可关
- [ ] 可添加视觉渠道；保存重开仍在
- [ ] 截图 OCR 仍走 Windows（改视觉 enabled 不影响）
- [ ] 翻译 Tab 零回归

- [ ] **步骤 3：文档回填并 Commit**

```powershell
git add docs/superpowers/specs/2026-07-15-services-drag-detail-ocr-design.md docs/superpowers/plans/2026-07-15-services-drag-detail-ocr.md README.md
git commit -m "docs: 回填服务拖拽/详情/OCR 配置实现状态"
```

---

## 自检

### 1. 规格覆盖度

| Spec 章节 | 任务 |
|---|---|
| §4 拖拽 | 任务 1 |
| §5.2 SettingTextarea | 任务 2–3 |
| §5.1/5.3/5.4 翻译详情 | 任务 4–5 |
| §6.2–6.4 数据与持久化 | 任务 6–8 |
| §6.3 视觉名单 | 任务 7 tokens |
| §6.5 Store API | 任务 8 |
| §6.1/6.6 OCR UI | 任务 10–11 |
| §2.1 i18n | 任务 9 |
| §2.2 范围外 | 未做 runtime 多引擎 / dedicated OCR |
| 验收与文档 | 任务 12 |

### 2. 占位符扫描

无 TODO/待定；关键代码块与命令已给出。

### 3. 类型一致性

- 前端实例：`OcrServiceInstance.type`；后端配置：`serviceType`（与翻译 `ServiceInstance`/`ServiceInstanceConfig` 对称）
- 合并函数名：`mergeBackendIntoOcrServices`
- 协议映射字段：`OcrServiceMeta.protocolId`（`'openai_chat' | 'claude_messages'`）
- 重置语义：统一 `resetPromptValue() === ''`
- Windows：`canDisable: false` / `canDelete: false` / store 强制 enabled

### 4. 与 OpenDesign 差异（实现时勿误拷）

| 点 | OpenDesign | 本计划 |
|---|---|---|
| dedicated OCR | 有 | **无** |
| Windows 可关 | canDisable true + 单活 | **不可关** |
| 多 OCR 启用 | ensureSingleOcrEnabled | **视觉可多开，仅存配置** |
| 外链 | `<a target=_blank>` | **`invokeOpenUrl`** |
| i18n / 真校验 | mock | **保留 shizi** |

---

## 执行说明

实现顺序严格按任务 1→12。每任务结束 commit。UI 任务在 typecheck 通过后，最终任务 12 做 WebView2 手工验收。
