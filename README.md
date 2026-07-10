# Shizi

基于大语言模型的 Windows 桌面翻译助手，灵感来自 macOS 的 Bob。

## 当前能力

当前 MVP 基于 Tauri 2 + Rust 后端 + Vue 3 前端实现，已经具备：

- 托盘常驻：关闭窗口会隐藏到托盘，通过托盘菜单退出应用。
- 手动输入翻译：在翻译弹窗输入文本后点击"翻译"。
- 可配置全局快捷键：设置页「全局快捷键」可修改、清空并保存划词翻译、截图 OCR 翻译、剪贴板翻译、显示主窗口和打开设置快捷键，保存成功后无需重启即可生效；取词翻译本轮仅保存绑定，不注册触发。
- OpenAI-compatible 流式翻译 provider：调用兼容 `/v1/chat/completions` 的流式接口。
- Claude / Anthropic 流式翻译 provider：调用 Anthropic Messages API 的 SSE 流式接口，支持 thinking 模式。
- Mock provider：用于无真实 API Key 的本地验证。
- 微软翻译 provider（Edge 引擎，免 Key 机器翻译）：调用 Edge 浏览器翻译接口，无需 API Key；机器翻译渠道当前仅微软翻译已对接，DeepL/Google/百度等保持开发中。
- 启动翻译弹窗与独立设置页：启动即显示翻译弹窗；设置页为独立窗口，可从翻译弹窗设置按钮或托盘「设置」打开。设置页含通用 / 翻译 / 快捷键 / 服务 / 历史 / 高级 6 个分类面板，支持多服务实例管理。翻译弹窗已去除 Windows 原生标题栏，改为自绘顶部工具栏（图钉 / 收藏 / 截图翻译 / 书签 / 设置）作为标题栏并支持拖拽，宽固定 420px、高度随内容自适应（最高 80% 屏幕高），视觉对齐 OpenDesign 原型。
- 配置实时同步：设置页保存服务启用 / 关闭后，会通过 `app-config:changed` 通知已打开的翻译弹窗同步结果卡片；非翻译中即时新增、删除、排序，翻译进行中保留正在输出的卡片，不新增未参与当前批次的服务卡片。
- 流式结果展示：Rust 后端通过 Tauri event 推送翻译状态和增量文本，前端实时渲染。
- 结果卡片长内容截断：翻译结果超过约 4-5 行时自动截断，底部渐隐遮罩 + 「展开全文」按钮，点击展开/收起。
- 输入原文限高：输入框超过最大高度（约 7 行）后内部滚动，不再撑高弹窗。
- 翻译取消与重试：流式翻译过程中可取消，失败或取消后可一键重试。
- OCR 错误指引：截图 OCR 失败（缺语言包 / 识别为空 / 区域过大等）时给出带阶段前缀与可操作指引的错误文案，并隐藏无意义的重试按钮。
- Token 用量展示：流式翻译结束时在译文下方显示 input → output token 数；可在设置页关闭采集。
- 翻译来源徽章：划词翻译显示「来自划词」、OCR 翻译显示「来自 OCR」，手动输入不显示；翻译结束/取消/失败/清空时徽章自动隐藏。
- 高级日志系统：前后端独立日志文件（后端 `Shizi.log` / 前端 `frontend.log`），运行时等级切换（error/warn/info/debug），API Key 与翻译正文脱敏，5MB 轮转 + 启动清理 7 天，一键导出 zip（含日志/配置快照/系统信息）。
- 翻译弹窗语言下拉：inline 搜索式 combobox（带搜索框、英文名双列、键盘 ↑↓/Enter/Esc 导航），非浮层实现不被弹窗 overflow 裁剪。
- 源语言自动检测：源语言选「自动检测」时，模型回传检测到的原文语言并显示在译文区右下角标签；翻译中显示「检测中…」。
- 默认目标语言：首次安装读操作系统语言，不在支持列表则回退英语；存量用户已选目标语言不受影响。

## 使用方式

### 手动翻译

1. 启动应用，默认显示翻译弹窗。
2. 在输入框输入要翻译的文本。
3. 点击"翻译"。

### 划词翻译

1. 在任意支持复制的应用中选中文本。
2. 按 `Alt+D`。
3. Shizi 会尝试读取选中文本并自动翻译。

> 当前划词复制只尽力保护纯文本剪贴板，不保证完整恢复图片、文件、HTML、RTF 等非文本剪贴板格式。

### 截图 OCR 翻译

1. 按 `Alt+E`，整屏冻结为 overlay 画面，鼠标变为十字。
2. 拖动选择要识别的矩形区域。
3. 松开鼠标后，Shizi 调用系统 OCR 识别选区文字并自动翻译。

> Esc、右键或选区过小（<3px）会取消本次截图，不进入翻译。Windows OCR 依赖系统语言包，中英混合需安装对应 OCR 语言包。

## 配置

设置页为独立窗口（Vue 3 + Tailwind v4 + reka-ui + @lucide/vue + @iconify/vue），当前支持：

- 通用（开机启动/主题/语言/关闭行为/窗口预创建策略/更新）
- 翻译（源语言/目标语言/默认服务实例/复制粘贴行为）
- 取词翻译、快捷键分组 / profile、导入导出仍未实现；word-lookup 绑定当前只保存不触发。
- 服务（内置 15 个渠道 + 自定义渠道，支持多实例、Key 管理、模型拉取、思维链深度、提示词编辑）
- 历史（OCR 翻译历史，含时间戳/源语种/目标语种/原文/译文）
- 高级（日志等级/导出/实验功能/匿名统计/配置导入导出/重置/关于）


### 服务协议与多结果翻译（v0.2）

- 服务列表默认展示 DeepSeek 与智谱 AI，默认关闭；启用后按列表顺序参与翻译。
- 服务实例通过 `protocol` 选择调用协议；协议 id 前后端统一为 `openai_chat` / `claude_messages` / `mock` / `microsoft_edge`，未知协议后端报错而非静默走 OpenAI 兼容。
- 前后端配置以 `config.json` 为事实来源：设置页挂载时从后端拉取，后端 `services` 为空则推前端覆盖（用于旧格式残留 / 首次启动），后端非空则按实例 id 合并（后端核心字段覆盖前端、前端独有字段如提示词保留）。
- 翻译弹窗按启用服务渲染多个结果卡，单个服务失败不影响其他服务；卡片图标按渠道 id（openai/deepseek/zhipu/claude/mock）区分。
- 翻译弹窗打开时即展示所有启用服务的占位卡片，翻译开始后原地刷新内容，无需等待首个结果返回。
- 设置页保存服务启用 / 关闭后，通过 `app-config:changed` 通知已打开的翻译弹窗同步结果卡片；非翻译中即时新增、删除、排序，翻译进行中保留正在输出的卡片，不新增未参与当前批次的服务卡片。
- 翻译弹窗输入为空或暂无翻译内容时，结果卡片默认收缩；开始翻译后对应卡片自动展开显示流式内容。
- 未对接渠道（gemini/deepl/google/baidu/youdao/tencent/volcengine/iflytek/moonshot/siliconflow）在添加 Dialog 标"开发中"badge、服务列表启用开关置灰、详情页顶部横幅提示。

> overlay 仍为纯静态 HTML/JS/CSS（`frontend/public/`），永久不迁；translate.html 已迁移为 Vue 3 入口（`frontend/src/popup/`），与设置页共享工程。

配置会保存到 Tauri 的应用配置目录下的 `config.json`。

> 注意：当前 MVP 会将 API Key 明文保存到本机配置文件。后续产品化阶段会迁移到 Windows Credential Manager / macOS Keychain / Linux Secret Service 等系统安全存储。

首次没有本地配置文件时，会从以下环境变量读取默认值：

```bash
SHIZI_LLM_PROVIDER=mock | openai_chat | claude_messages
SHIZI_TARGET_LANG=中文
SHIZI_OPENAI_API_KEY=...
SHIZI_OPENAI_BASE_URL=https://api.openai.com/v1
SHIZI_OPENAI_MODEL=gpt-4o-mini
SHIZI_OPENAI_TIMEOUT_SECS=60
SHIZI_CLAUDE_API_KEY=...
SHIZI_CLAUDE_BASE_URL=https://api.anthropic.com
SHIZI_CLAUDE_MODEL=claude-haiku-4-5
SHIZI_CLAUDE_TIMEOUT_SECS=60
SHIZI_CLAUDE_ENABLE_THINKING=false
```

本地 mock 模式示例：

```bash
SHIZI_LLM_PROVIDER=mock npm run tauri dev
```

## 当前限制

以下能力尚未实现：

- Slint 原生高性能翻译弹窗。
- 取词翻译、快捷键分组 / profile、导入导出仍未实现；word-lookup 绑定当前只保存不触发。 系统安全存储。
- 快捷键自定义、深色模式、部分面板操作（历史/语音输入等）——已在设置页 UI 中以「实现中」标签预留。
- 后端日志文件名为 `Shizi.log`（tauri-plugin-log 按 `productName` 默认，不支持自定义）；API Key 明文保存到 config.json（MVP，后续迁移系统安全存储）。

截图 OCR 已落地，但存在以下 MVP 已知限制：

- 多显示器下，`Alt+E` 抓帧按光标定位显示器，但 overlay 窗口默认建在主屏，光标在副屏时可能错位。
- 缩放比例取主窗口近似目标显示器，混合 DPI 多屏下框选坐标可能不准。
- 锁屏 / 屏保 / 安全桌面 / 远程会话下 DXGI 抓帧可能失败。

## 命令

```bash
npm install               # 首次需装前端依赖（Vite/Vue/Tailwind/shadcn-vue）
npm run tauri dev         # 开发模式（拉起 Vite dev server + 后端）
npm run tauri build       # 生成 release 安装包
npm run dev               # 仅启动前端 Vite dev server（无 Tauri 容器，invoke 不可用）
npm run build             # 仅构建前端到 frontend/dist/
npm run typecheck         # vue-tsc 类型检查
npm run test              # vitest 单测
cd src-tauri && cargo build           # 仅构建后端 debug
cd src-tauri && cargo build --release # 仅构建后端 release
cd src-tauri && cargo test            # 后端单测
cd src-tauri && cargo clean           # 清理 Rust 编译缓存
```

> `npx tauri dev` 也可代替 `npm run tauri dev` 执行。
