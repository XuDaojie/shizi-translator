# 应用国际化与可扩展语言包设计规格

**日期：** 2026-07-11
**状态：** 已批准
**范围：** 设置页、翻译弹窗、托盘、Windows 窗口标题、翻译源语言与目标语言
**不在范围：** 无可见文案的截图 overlay、日志内容、用户输入、模型名、服务商原始错误详情

## 1. 背景与目标

当前「通用 > 外观 > 界面语言」只有简体中文和 English 两个选项，配置仅保存在前端状态中，没有驱动界面文案。设置页、翻译弹窗和托盘仍使用硬编码中文。翻译源语言与目标语言列表也只包含 9 种语言，并在两个前端文件中重复维护；Microsoft Edge provider 只映射现有 9 种语言，未知目标语言会静默降级为英语。

本次实现以下目标：

1. 提供 8 种内置界面语言和「自动检测」。
2. 默认界面语言为 `auto`，根据操作系统 locale 解析；不支持时回退简体中文。
3. 保存后立即同步设置页、翻译弹窗、托盘及窗口标题，无需重启应用。
4. 使用轻量、无第三方依赖的键值字典实现国际化。
5. 简体中文和英语使用静态导入，其他内置语言按需加载。
6. 支持用户在应用配置目录的 `lang/` 中添加或覆盖 JSON 语言包。
7. 将翻译语言扩充到原型中的 19 种实际语言，并完整接入 LLM 与 Microsoft Edge provider。
8. 删除前端重复翻译语言列表，避免设置页、弹窗和历史面板不一致。

## 2. 已批准的界面语言

内置界面语言如下：

| 配置值 | 显示名 |
| --- | --- |
| `auto` | 自动检测 |
| `zh-CN` | 简体中文 |
| `zh-TW` | 繁體中文 |
| `en-US` | English |
| `ja-JP` | 日本語 |
| `ko-KR` | 한국어 |
| `fr-FR` | Français |
| `de-DE` | Deutsch |
| `es-ES` | Español |

`auto` 是持久化配置值，不在首次启动时替换成检测结果。每次启动和用户重新选择 `auto` 时都重新读取操作系统 locale。

解析规则：

1. 用户明确选择某个可用 locale 时，直接使用该语言。
2. 配置为 `auto` 时，先尝试精确匹配已安装的用户语言包，再匹配内置语言映射。
3. 中文 locale 中包含 `Hant`、`TW`、`HK` 或 `MO` 时映射为 `zh-TW`，其余中文映射为 `zh-CN`。
4. 英语、日语、韩语、法语、德语和西班牙语的地区变体映射到对应内置 locale。
5. 操作系统语言不在可用语言列表、locale 读取失败或配置非法时，回退 `zh-CN`。
6. 运行期间不监听 Windows 系统语言变化；下次启动或重新选择 `auto` 时生效。

用户语言包会扩展可用界面语言列表。用户包与操作系统 locale 精确匹配时，`auto` 可以选择该用户语言；不对多个用户自定义地区变体做猜测式主语言匹配。

## 3. 配置模型

后端 `AppConfig` 新增：

```rust
#[serde(default = "default_interface_language")]
pub interface_language: String,
```

默认值为 `"auto"`，序列化字段为 `interfaceLanguage`。该字段进入现有 `config.json`，是界面语言选择的唯一持久化事实来源。前端 `AppConfig` 和 `AppSettings.general.language` 同步扩展类型。

配置保存仍使用现有 `save_app_config`。不新增 localStorage 语言配置，不引入第二套持久化来源。

旧界面语言或翻译语言代码不迁移。非法值按本规格的回退规则处理：

- 非法界面语言回退 `zh-CN`。
- 非法默认源语言重置为 `auto`。
- 非法默认目标语言重新按操作系统语言映射到新翻译语言表；无法映射时回退 `zh-CN`。
- 历史记录中的旧语言代码不改写；无法查到名称时显示原始代码。

用户明确选择的自定义 locale 在对应语言包被删除后视为不可用，运行时回退 `zh-CN`；下一次保存设置时写回有效选择。

## 4. 语言包格式与存储

### 4.1 内置语言包

内置语言包使用统一 JSON 格式，作为源码随应用打包：

```text
frontend/src/i18n/
  index.ts
  loaders.ts
  locales/
    zh-CN.json
    zh-TW.json
    en-US.json
    ja-JP.json
    ko-KR.json
    fr-FR.json
    de-DE.json
    es-ES.json
```

文件结构：

```json
{
  "schemaVersion": 1,
  "locale": "en-US",
  "name": "English",
  "messages": {
    "common.save": "Save",
    "settings.general.title": "General",
    "popup.status.ready": "Ready",
    "tray.translate": "Translate",
    "tray.settings": "Settings",
    "tray.quit": "Quit",
    "tray.tooltip": "Shizi - Translation Assistant"
  }
}
```

`messages` 使用扁平、稳定、按领域分组的键。简体中文内置包定义完整键集合，是最终回退字典和完整性测试基准。

### 4.2 用户语言包

用户语言包位于 Tauri 应用配置目录下的 `lang/`：

```text
<app_config_dir>/lang/
  it-IT.json
  en-US.json
```

用户包可执行两种操作：

- 使用新 locale 新增界面语言。
- 使用相同 locale 部分或全部覆盖内置语言包。

用户包允许只包含部分 `messages`。加载优先级固定为：

```text
用户同 locale 语言包
  -> 同 locale 内置语言包
  -> 内置 zh-CN 语言包
```

覆盖 `zh-CN` 时，缺失键回退原始内置简体中文包，不能回退到覆盖包自身。

### 4.3 安全校验

语言包仅接受 JSON，不执行 JavaScript 或其他代码。校验规则：

1. 文件名必须为 `<locale>.json`。
2. `schemaVersion` 必须为整数 `1`。
3. `locale` 必须与文件名一致，并符合受限 BCP 47 形式。
4. `name` 必须是非空字符串。
5. `messages` 必须是扁平的字符串到字符串映射。
6. 单文件最大 1 MB。
7. 未知消息键视为校验错误，避免拼写错误被静默忽略。
8. 缺失消息键合法，由回退链补齐。
9. 无效文件不进入语言选项，也不能替换当前有效字典；错误信息在设置页显示。

扫描用户目录时逐文件读取并校验，仅长期保留语言包元数据和当前生效字典；扫描过程不缓存所有用户字典。

## 5. 加载策略

国际化只有一套键协议、覆盖规则和回退规则，静态与动态只是加载策略差异。

### 5.1 前端

- `zh-CN` 静态导入，始终作为最终回退。
- `en-US` 静态导入，覆盖最常见的非中文环境。
- 其余 6 种内置语言通过显式 loader map 使用 `import()` 动态加载。
- 用户新增或覆盖包通过 Tauri command 按当前 locale 获取。
- 前端只持有静态导入的内置简体中文与英文、当前动态内置包和当前用户覆盖包。
- 切换语言时释放上一份动态包的应用引用；实际回收时机由 WebView2 JavaScript GC 决定。

显式 loader map 避免运行时拼接任意路径：

```ts
const builtinLoaders = {
  'zh-TW': () => import('./locales/zh-TW.json'),
  'ja-JP': () => import('./locales/ja-JP.json'),
  'ko-KR': () => import('./locales/ko-KR.json'),
  'fr-FR': () => import('./locales/fr-FR.json'),
  'de-DE': () => import('./locales/de-DE.json'),
  'es-ES': () => import('./locales/es-ES.json'),
}
```

### 5.2 后端与托盘

Rust 使用同一组内置 JSON 源文件在编译时嵌入必要内容，并按当前 locale 解析托盘键。用户覆盖包由后端从 `lang/` 读取，因此托盘与前端使用相同的覆盖优先级。

后端不把全部语言包解析进运行时状态。内置 JSON 作为程序资源存在，当前 locale 的托盘键和用户覆盖包按需解析。

### 5.3 语言包管理操作

「界面语言」设置旁提供两个图标按钮：

- 打开语言包目录。
- 重新扫描语言包。

应用启动时自动扫描。用户新增、修改或删除 JSON 后点击刷新，设置选项、当前页面和托盘立即更新。不增加常驻文件监听器，不引入文件监听依赖。

## 6. 前端国际化 API

`frontend/src/i18n/index.ts` 提供每个 WebView 内的轻量响应式状态：

```ts
type MessageParams = Record<string, string | number>

interface I18nState {
  locale: Readonly<Ref<string>>
  t(key: MessageKey, params?: MessageParams): string
  setLocale(locale: string): Promise<void>
}
```

要求：

- `t()` 按用户包、同 locale 内置包、内置简中顺序查找。
- 支持简单 `{name}` 参数插值，不实现 ICU 复数语法。
- 日期、时间和数字使用浏览器原生 `Intl`，locale 与当前界面语言一致。
- 设置页、翻译弹窗、toast、tooltip、placeholder、空态、状态、`aria-label` 和窗口标题均使用消息键。
- 运行时状态保存“状态键 + 参数”，不保存已经翻译的状态句子，以便切换语言后立即重算。
- 已经显示的临时 toast 不追溯替换，后续 toast 使用新 locale。
- 用户输入、服务实例名称、模型名、原文、译文和服务商原始错误详情保持原样。
- 应用自带错误标题和说明进入字典。

截图 overlay 当前只有 canvas、遮罩和框选区域，没有可见文案，因此不接入字典或语言事件。其日志保持现状。

## 7. 运行时同步

后端负责解析实际 locale，并提供初始化所需的只读 command。保存配置或刷新语言包成功后执行：

1. 持久化并规范化 `interfaceLanguage`。
2. 解析实际 locale 和当前用户覆盖包。
3. 更新托盘三个菜单项和 tooltip。
4. 广播界面语言变更事件；即使 locale 未变化但覆盖包内容变化，也必须广播新 revision。
5. 设置页和翻译弹窗按事件重新加载当前字典。
6. 更新各页面 `<html lang>` 和 Windows 窗口标题。

事件只携带 locale 与 revision 等小型元数据，不广播完整字典。每个打开的 WebView 通过 command 获取当前用户覆盖包，内置包走静态或动态 loader。

切换语言不重载 WebView，不清空输入，不中断翻译，不重建结果卡。托盘使用固定 ID，通过 Tauri 原生菜单更新 API 替换菜单与 tooltip。

所有内置界面语言均为从左到右布局，不翻转应用布局。翻译原文和译文内容容器使用 `dir="auto"`，使阿拉伯语等内容按文本自身方向显示。

## 8. 翻译语言目录

源语言包含 `auto` 加 19 种实际语言；目标语言不包含 `auto`。

| 代码 | 本地名称 | 英文名称 | Microsoft Edge code |
| --- | --- | --- | --- |
| `auto` | 自动检测 | Auto Detect | 省略 `from` |
| `zh-CN` | 简体中文 | Chinese (Simplified) | `zh-Hans` |
| `zh-TW` | 繁體中文 | Chinese (Traditional) | `zh-Hant` |
| `en` | English | English | `en` |
| `ja` | 日本語 | Japanese | `ja` |
| `ko` | 한국어 | Korean | `ko` |
| `fr` | Français | French | `fr` |
| `de` | Deutsch | German | `de` |
| `es` | Español | Spanish | `es` |
| `pt` | Português | Portuguese | `pt` |
| `ru` | Русский | Russian | `ru` |
| `it` | Italiano | Italian | `it` |
| `nl` | Nederlands | Dutch | `nl` |
| `pl` | Polski | Polish | `pl` |
| `tr` | Türkçe | Turkish | `tr` |
| `ar` | العربية | Arabic | `ar` |
| `th` | ภาษาไทย | Thai | `th` |
| `vi` | Tiếng Việt | Vietnamese | `vi` |
| `id` | Bahasa Indonesia | Indonesian | `id` |
| `hi` | हिन्दी | Hindi | `hi` |

前端只维护一份翻译语言元数据，设置页、翻译弹窗和历史面板共同引用。下拉第一列始终显示本地名称，第二列通过界面语言字典显示本地化语言名称。

LLM prompt 使用与界面语言无关的规范语言名称或稳定代码，切换 UI 语言不能改变翻译请求语义。

Microsoft Edge provider 必须为表中所有语言增加显式正向映射和检测结果反向映射。未知源语言和未知目标语言均返回明确的不可重试配置错误；未知目标语言不得再静默降级为英语。

不兼容旧翻译语言代码：`en-US`、`ja-JP`、`ko-KR`、`fr-FR`、`de-DE`、`es-ES`、`ru-RU` 不做别名转换。

## 9. 错误处理

- 操作系统 locale 读取失败：界面回退 `zh-CN`，不阻止启动。
- 动态内置包加载失败：回退内置 `zh-CN`，记录错误日志。
- 用户覆盖包读取或校验失败：保留当前有效字典，设置页显示文件级错误。
- 当前自定义包被删除：刷新后回退 `zh-CN`，托盘和窗口同步更新。
- Tauri 语言事件广播失败：配置仍已保存；返回保存错误并记录日志，避免误报全局已同步。
- 托盘更新失败：不阻止 WebView 切换，记录错误并在设置页显示刷新失败。
- 外部服务错误：本地化错误标题，保留原始详情用于排障。

## 10. 测试与验收

### 10.1 Rust 单元测试

- `interfaceLanguage` 默认值为 `auto`，序列化为 camelCase。
- 操作系统 locale 精确映射、地区变体映射、中文脚本映射和未知语言回退。
- 非法界面语言回退 `zh-CN`。
- 非法默认源语言回退 `auto`。
- 非法默认目标语言重新映射 OS，无法映射时回退 `zh-CN`。
- 用户语言包文件名、schema、locale、name、messages、大小和未知键校验。
- 用户包覆盖、部分覆盖、删除恢复和三层回退。
- Microsoft Edge 19 种语言正向映射与反向映射。
- 未知 Edge 源语言和目标语言返回错误，不降级。
- 托盘消息解析和覆盖优先级。

### 10.2 前端单元测试

- 8 份内置字典的消息键集合与 `zh-CN` 完全一致。
- 中英文静态加载，其他内置语言动态 loader 映射完整。
- 用户新增语言、同 locale 覆盖和缺键回退。
- `t()` 参数插值、缺键回退和 locale 切换。
- 翻译语言目录包含 19 种目标语言，源语言额外包含且仅包含一个 `auto`。
- 设置页、弹窗和历史面板引用同一翻译语言目录。
- 配置投影和后端同步包含 `interfaceLanguage`。

### 10.3 验证命令

```bash
npm run test
npm run typecheck
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo build
```

### 10.4 手动验收

1. 首次启动默认选择「自动检测」。
2. 8 种内置语言均可切换，设置页、翻译弹窗、托盘和窗口标题立即更新。
3. 不支持的系统语言回退简体中文。
4. 翻译进行中切换界面语言，不中断翻译且状态文案立即更新。
5. 源语言下拉包含自动检测和 19 种语言，目标语言包含 19 种语言且无自动检测。
6. 下拉第一列显示本地名称，第二列随界面语言变化。
7. 19 种语言通过 LLM 请求使用正确语言语义。
8. 19 种语言通过 Microsoft Edge 使用正确 code，不出现英语静默降级。
9. 打开语言包目录，添加新 locale 后点击刷新，新语言立即出现在选项中。
10. 添加部分 `en-US` 覆盖包后点击刷新，覆盖键更新，缺失键仍使用内置英文。
11. 删除覆盖包并刷新，恢复内置文案。
12. 无效、超大或含未知键的语言包被拒绝，并显示具体错误。
13. 阿拉伯语原文或译文按 RTL 显示，但应用布局保持 LTR。

## 11. 文档同步

编码执行完成且测试通过后，必须同步：

- `README.md`：界面语言、自动检测、即时切换、翻译语言和用户语言包说明。
- roadmap：国际化能力完成状态。
- 架构文档：语言包加载、配置、事件和托盘同步边界。
- `AGENTS.md` 与 `CLAUDE.md`：保持内容一致，补充国际化架构关键点。
- 对应实现计划复选框和完成状态。

文档同步完成后才能进入开发分支收尾流程。

## 12. 明确不做

- 不引入 `vue-i18n`、日期库或文件监听依赖。
- 不执行用户提供的 JS/TS 语言包。
- 不监听运行期间的 Windows 系统语言变化。
- 不为没有可见文案的 overlay 增加国际化运行时。
- 不迁移或转换旧翻译语言代码。
- 不实现在线语言包市场、下载、签名或自动更新。
- 不实现 ICU MessageFormat、复杂复数规则或整套 RTL 界面镜像。
