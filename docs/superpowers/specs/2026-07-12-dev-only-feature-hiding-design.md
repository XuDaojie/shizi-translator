# 开发中功能 dev 可见 / release 隐藏 设计规格

**日期：** 2026-07-12
**状态：** 已批准
**范围：** 设置页服务详情的 wip 功能块（思维链、反思、主题、自动检查更新）、语言包工具（打开目录/刷新）、高级面板隐私与关于整块、未对接渠道（添加对话框、服务列表、详情页）
**不在范围：** 侧边栏 badge wip 机制（当前未使用）、后端翻译行为、config.json 数据删改、dead code elimination；界面语言下拉本身在 release 保留

## 1. 背景与目标

设置页有部分功能标了「开发中」（`SettingRow status="wip"` 徽标 + 未对接渠道 amber 标记），在 dev 与 release 包中行为一致--release 用户也能看到这些未完成功能。希望 release 包不暴露开发中功能，仅开发界面可见。

当前三类「开发中」标记现状：

1. `SettingRow status="wip"` 共 4 处：思维链、反思（ServicesPanel）、主题、自动检查更新（GeneralPanel）。功能已渲染可交互，仅挂 amber 徽标。
2. 未对接渠道 `isDeveloping(type)`（ServicesPanel）：`protocols` 为空的服务类型（deepl/google/baidu 等），在添加对话框 / 服务列表 / 详情页标 amber 且禁用启用开关。
3. 侧边栏 `badge: 'wip'`：机制存在但当前 0 处使用。

目标：

1. wip 功能块在 release 包不渲染设置 UI，dev 下保持现状（含「开发中」徽标）。
2. 未对接渠道在 release 包的添加对话框 / 服务列表 / 详情页不显示，dev 下保持现状。
3. 已配置值在 release 仍生效（思维链等后端行为不变），仅隐藏设置入口。
4. config.json 数据不删，dev 切回仍可见。
5. 环境判据基于 Vite 编译期常量，无需改构建配置。

## 2. 决策汇总

| 决策点 | 结论 |
| --- | --- |
| 覆盖范围 | wip 功能块 + 未对接渠道（侧边栏 badge 不在范围） |
| 未对接渠道存量实例 | release 全部隐藏，config.json 数据保留 |
| wip 已配值运行时行为 | 保留已配值生效，仅前端隐藏 UI，后端不动 |
| 隐藏机制 | `useDevMode` composable + `DevOnly` 组件 + 渠道过滤 |
| 环境判据 | `import.meta.env.DEV`（dev=true，tauri build=false） |
| DCE | 不做，运行时 `v-if` 隐藏 |

## 3. 环境判据与 DevOnly 组件

### 3.1 useDevMode composable

文件：`frontend/src/settings/composables/useDevMode.ts`

```ts
export function useDevMode(): boolean {
  return import.meta.env.DEV
}
```

集中环境判据。`import.meta.env.DEV` 是 Vite 编译期常量：

- `npm run tauri dev` / `npm run dev` / 走 localhost:5173 的 dev 模式 exe -> `true`
- `npm run tauri build`（`vite build` 生产构建 + 打包）-> `false`

封装为 composable 便于测试 mock 与未来扩展（如运行时调试开关）。

### 3.2 DevOnly 组件

文件：`frontend/src/settings/components/DevOnly.vue`

```vue
<script setup lang="ts">
import { useDevMode } from '../composables/useDevMode'
const isDev = useDevMode()
</script>

<template>
  <template v-if="isDev"><slot /></template>
</template>
```

用 `<template v-if>` 不产生额外 DOM 节点。`isDev=false` 时不渲染 slot 内容。

### 3.3 隐藏方式

运行时 `v-if` 隐藏，不做 dead code elimination。原因：「已配值生效」决策要求思维链等代码仍在 bundle 中运行（`DevOnly` 只隐藏设置 UI，不移除功能代码）。

## 4. wip 功能块隐藏

用 `<DevOnly>` 包裹以下 wip 入口，保留各处 `status="wip"`（dev 下徽标照常显示；语言包图标按钮无单独徽标）：

| 功能 | 文件 | 包裹对象 |
| --- | --- | --- |
| 思维链 | `frontend/src/settings/panels/ServicesPanel.vue` | 整个 SettingGroup（思维链分组） |
| 反思 | `frontend/src/settings/panels/ServicesPanel.vue` | 单个 SettingRow（prompts 组内仅包反思这一项，系统 / 翻译提示词不受影响） |
| 主题 | `frontend/src/settings/panels/GeneralPanel.vue` | SettingRow |
| 自动检查更新 | `frontend/src/settings/panels/GeneralPanel.vue` | SettingRow |
| 语言包目录 / 刷新 | `frontend/src/settings/panels/GeneralPanel.vue` | 界面语言行内图标按钮 + 语言包错误提示；语言下拉 release 保留 |
| 隐私 | `frontend/src/settings/panels/AdvancedPanel.vue` | 整个 SettingGroup（含匿名统计） |
| 关于 | `frontend/src/settings/panels/AdvancedPanel.vue` | 整个 SettingGroup |

职责分离：

- `status="wip"` = dev 下的「开发中」提示徽标
- `<DevOnly>` = release 隐藏

## 5. 未对接渠道隐藏

### 5.1 添加渠道对话框

添加渠道对话框的渠道按钮网格（`v-for="svc in mergedServices"`）在 release 过滤掉 `protocols.length === 0` 的未对接渠道。dev 下不过滤，保持现状（含 amber 标记）。`ChannelCombobox` 仅用于自定义渠道创建（均为 `openai_chat` 协议），不涉及未对接渠道，无需处理。

### 5.2 服务列表

`filteredInstances` computed 叠加过滤条件：`isDev || !isDeveloping(type)`。未对接渠道实例在 release 不渲染。dev 下保持现状。

### 5.3 详情页选中态回退

`activeInstanceId` 当前初始化为 `props.state.services[0]?.id`。改为取第一个**可见**实例：

- 初始化（`activeInstanceId` 初值）：第一个满足 `isDev || !isDeveloping(type)` 的实例 id，无则空字符串。
- 删除当前选中实例后的回退（现有 `activeInstanceId.value = props.state.services[0]?.id` 处）：改为第一个可见实例。
- 无可见实例：详情页走已有空态分支（`v-else-if="activeInstance && activeService"` 不成立时的兜底）。

### 5.4 数据保留

config.json 中的未对接渠道实例不删除。release 下 UI 不渲染，dev 切回仍可见可编辑。

## 6. 边界与不变量

1. config.json 数据不删--未对接渠道实例保留。
2. 已配值生效--思维链等后端行为不变，`DevOnly` 只隐藏设置 UI。
3. 反思字段后端本就不读（`TranslationPromptConfig` 无 reflection 字段，`build_batch_requests` 不传），隐藏 UI 无运行时影响。
4. 侧边栏 `badge: 'wip'` 不在本次范围。
5. `cargo build --release` 生成的 dev 模式 exe（走 localhost:5173）前端 `DEV=true`，开发中功能可见--符合「开发界面」语义。

## 7. 测试策略

项目现有前端测试惯例：vitest 测纯函数 / store（`config-io.test.ts`、`service-validation.test.ts`、`settings.test.ts`），`vue-tsc` 类型检查，Tauri dev 手动验证。本次按此惯例：

### 7.1 useDevMode

单测：mock `import.meta.env.DEV` 为 true / false，断言返回值。注：vitest 默认 `mode=test`，`import.meta.env.DEV` 为 true，测 false 分支需 mock。

### 7.2 DevOnly

逻辑极简（`v-if` 透传 slot），按项目惯例不强制单测；若引入 `@vue/test-utils` 则覆盖 isDev=true 渲染 slot、isDev=false 不渲染。默认通过 `npm run typecheck` + Tauri dev 手动验证（dev 可见、`npm run build` 产物不可见）。

### 7.3 渠道过滤

- release（isDev=false）：未对接渠道不出现在 `filteredInstances` 与添加对话框 options；`activeInstanceId` 回退到第一个可见实例。
- dev（isDev=true）：未对接渠道照常出现。

优先以 `filteredInstances` 的纯函数化抽取支持单测；无法纯函数化的部分靠手动验证。

### 7.4 回归

现有 `config-io.test.ts` / `service-validation.test.ts` 纯函数不受影响。`npm run typecheck` 通过。

## 8. 不做的事（YAGNI）

1. 不做 dead code elimination / 动态导入。
2. 不动后端翻译逻辑。
3. 不处理侧边栏 `badge: 'wip'`（当前未使用，将来启用时再纳入）。
4. 不删 config.json 数据。
5. 不新增运行时调试开关（composable 仅封装编译期常量）。

## 9. 相关文件

新增：

- `frontend/src/settings/composables/useDevMode.ts`
- `frontend/src/settings/components/DevOnly.vue`

修改：

- `frontend/src/settings/panels/ServicesPanel.vue`（思维链 DevOnly、反思 DevOnly、`filteredInstances` 过滤、添加对话框 `mergedServices` 过滤、`activeInstanceId` 回退）
- `frontend/src/settings/panels/GeneralPanel.vue`（主题 DevOnly、自动检查更新 DevOnly）

测试：

- `frontend/src/settings/composables/useDevMode.test.ts`
