# 设置页视觉打磨设计

> 日期：2026-07-02
> 状态：已确认，待编写实现计划
> 策略：一次搬运、单页（删除旧设置页，用 OpenDesign 原型整套重写）

## 1. 背景与目标

OpenDesign 产出了设置页高保真原型（`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi`），技术栈与 shizi 主项目高度一致（Vue 3 + TS + Tailwind + reka-ui + Iconify）。本任务按原型重写 shizi 的设置页，做视觉打磨。

原型是当前设置页的近乎完整超集：从单 provider 二选一升级到多服务实例模型，并新增通用 / 翻译 / 快捷键 / 服务 / 历史 / 高级 6 个分类面板。原型自身声明「仅设置页原型，后端能力需在接入时实现」。

**目标**：

1. 用原型整套 UI 重写 `frontend/src/settings/`，视觉与交互对齐原型。
2. 已实现字段接 Tauri `save_app_config` 真正生效；未实现字段保留 UI、可交互、本地持久化，并打「实现中」标签。
3. 不新增后端 command，不破坏现网可用功能。

## 2. 范围

### 删除

- 旧 `frontend/src/settings/App.vue` 及 7 个子组件（`TargetLangSection` / `ProviderSelect` / `OpenAiSection` / `ClaudeSection` / `StrategySection` / `SaveBar` / `ApiKeyField`）。
- 现有 `frontend/src/components/ui/*`（仅旧设置页引用，删除旧页后无破坏面；translate.html / overlay.html 是纯静态页，不碰 Vue）。

### 新增（从原型搬入并适配）

- `frontend/src/settings/`：`SettingsPage` / `SettingsLayout` / `SettingsSidebar` + `components/` + `panels/` + `stores/settings.ts` + `tokens.ts` + `types.ts`。
- `frontend/src/components/ui/`：原型的 button / badge / dialog / input / select / switch / toast / tooltip。
- `frontend/src/lib/config.ts`：新增 `projectToAppConfig(state)` 投影函数。

### 不做（YAGNI）

- 不接 Vue Router（`SettingsPage` 已用 `window.history` hash 切分类，够用）。
- 不做 Iconify 离线包（在线 `@iconify/vue` 即可）。
- 不新增任何后端 command。
- 不做老用户数据迁移、不调 `get_app_config` 反向 seed（见 §4）。

## 3. 技术适配（原型 → shizi）

三处坑，需机械改写而非直接复制：

### 3.1 Tailwind v3 → v4

原型用 Tailwind v3（`tailwind.config.ts` + `@tailwind base`）；shizi 用 v4 CSS-first（`@tailwindcss/vite` + `@theme inline`）。

- **不**搬原型的 `style.css` 与 `tailwind.config.ts`。
- 把原型 HSL 调色板（蓝色 `--primary: 222 70% 48%`、amber/emerald 状态色）写进 shizi 现有 `frontend/src/styles/main.css` 的 `:root` / `.dark`，保留现有 `@theme inline` 映射结构。现有 main.css 注释「待原型图定稿后微调配色」即此次落地。
- 补 token：`--sidebar-width: 240px`、`--content-max-width: 720px`。
- 补 utility / keyframes：`scrollbar-thin`、`toast-slide-in/out`、`api-key-progress`。

### 3.2 图标包

原型 `lucide-vue-next` → shizi 的 `@lucide/vue`。机械改写所有图标 import（名称一致）。

### 3.3 UI 原子

原型的 `components/ui/` 替换 shizi 现有生成的原子。注意 `cn()` / `cva` 已在 shizi `lib/utils.ts` 就绪，沿用。

## 4. 数据模型与桥接

### 4.1 前端 store

原型 `useSettings`（`stores/settings.ts`）**原样搬入**：localStorage 持久化、多实例 `ServiceInstance[]`、dirty 追踪、save/discard/reset、服务增删改拖拽、OCR 历史。

**唯一真相源 = localStorage**。不调 `get_app_config` 读取，删除旧 `load()` 与 `invokeGetAppConfig` 调用。首次无 localStorage → 用原型默认值 seed（openai/claude 各一个空实例，默认 baseUrl/model 与后端 `from_env` 默认一致）。理由：无需兼容老用户，不做反向迁移。

### 4.2 桥接：为什么需要

前后端数据模型不对等：

- 前端：多实例 `ServiceInstance[]`，14 种渠道，同渠道可多实例，含思维链 / 提示词 / 反思。
- 后端：单 provider `AppConfig`，`provider` 二选一 + 一组配置，唯一保存接口 `save_app_config(AppConfig)`。

后端只有一个保存接口，本阶段不新增 command。保存时必须把前端「当前默认服务实例」压扁成后端能接受的 `AppConfig`——这就是 `projectToAppConfig`。没有它，已实现字段（key / targetLang 等）落不进后端、不会真正生效。

### 4.3 `projectToAppConfig(state): AppConfig` 投影规则

取 `state.translation.defaultServiceInstanceId` 指向的实例：

| 默认实例 type | 映射 |
|---|---|
| `openai-compatible` | `provider='openai-compatible'` + `{apiKey, baseUrl, model, timeoutSeconds}` |
| `claude` | `provider='claude'` + `{apiKey, baseUrl, model, timeoutSeconds, enableThinking = chainOfThought !== 'off'}` |
| 其他（DeepL / Gemini / 百度…） | `provider` 取**上次成功保存的缓存值**（store 内存持有 `lastSavedProvider`，首次为 `openai-compatible`），本地照存；toast 提示「默认服务 X 暂未接入后端，已本地保存」 |

其余已实现字段：

- `targetLang ← state.translation.defaultTargetLang`
- `popupPrecreate / overlayPrecreate / collectUsage ←` 对应已实现字段（归属见 §5）。

`timeoutSeconds`：原型实例无此字段，用后端默认 60。若后续需要可扩 `ServiceInstance`。

### 4.4 保存流程

```
save():
  projected = projectToAppConfig(state)
  err = validateConfig(projected)   // 复用现有纯函数
  if err: toast(err); return
  if isTauriReady():
    saved = await invokeSaveAppConfig(projected)
    store.save()                     // baseline 落定
    toast('配置已保存')
  else:
    store.save()                     // 纯浏览器预览，仅本地
    toast('Tauri 未就绪，仅本地保存')
```

## 5. 字段实现状态清单

| 面板 | 已实现（接后端，无标签） | 实现中（本地 + 标签） |
|---|---|---|
| 通用 | `popupPrecreate` / `overlayPrecreate`（「窗口策略」分组，已确认归此面板） | 开机启动、托盘、关闭行为、主题、语言、更新通道 |
| 翻译 | `defaultTargetLang` | 源语言、复制/粘贴/音标/备选、取词延迟、历史上限、默认服务选择 |
| 快捷键 | `Alt+T` / `Alt+O` 后端硬编码 → **只读展示当前绑定** | 其余绑定可编辑（无后端 command） |
| 服务 | 默认实例为 openai-compatible / claude 时其 key/baseUrl/model/timeout | 多实例、其余 12 种渠道、思维链、系统/翻译/反思提示词、拉取模型、Key 校验 |
| 历史 | — | 全部（无后端持久化，整面板打标） |
| 高级 | `collectUsage` | 日志等级、实验功能、导入/导出、重置、关于 |

## 6. 「实现中」标签约定

- **视觉**：复用原型 `Badge variant="warning"`（amber），文案「实现中」。sidebar 已用同款标「开发中」，保持一致。
- **落点优先级**：SettingRow 级（单行打标）> 面板级（如历史整面板打标）。
- **交互**：可点可填，值入本地 store；保存时不进 `projectToAppConfig`。
- **快捷键面板特例**：后端硬编码的两条只读展示当前绑定 + 标签，不可编辑；其余可编辑但属实现中。

## 7. 测试

- 保留 `validateConfig` 单测。
- **新增 `projectToAppConfig` 纯函数单测**：
  - 默认实例为 openai-compatible → provider 与字段映射正确。
  - 默认实例为 claude → provider 与 `enableThinking`（chainOfThought 映射）正确。
  - 默认实例为非支持类型（如 deepl）→ provider 保持不变（fallback）。
  - 默认实例为空 → 安全降级。
  - `targetLang` / `popupPrecreate` / `overlayPrecreate` / `collectUsage` 透传正确。
- `vue-tsc` typecheck 通过。
- `tauri dev` 手动验证：保存后重启，已实现字段生效；实现中字段本地保留、刷新仍在。

## 8. 风险

- **Tailwind v4 token 迁移**：原型 HSL 值搬进 oklch 体系时需核对显色。缓解：保留原型 HSL 数值原样写入 `:root`，`@theme inline` 已用 `hsl(var(--x))` 桥接，不混 oklch。
- **两份配置漂移**：前端 localStorage 与后端 `config.json` 各存一份。保存时同步写两边；读取只信 localStorage。用户手改 `config.json` 或环境变量覆盖时前端不感知——dev 版本可接受。
- **图标包 API 差异**：`@lucide/vue` 与 `lucide-vue-next` 命名一致但导入路径不同，需全量改写。缓解：typecheck 兜底。
