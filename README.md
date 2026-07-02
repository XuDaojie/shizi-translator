# Shizi

基于大语言模型的 Windows 桌面翻译助手，灵感来自 macOS 的 Bob。

## 当前能力

当前 MVP 基于 Tauri 2 + Rust 后端 + Vue 3 前端实现，已经具备：

- 托盘常驻：关闭窗口会隐藏到托盘，通过托盘菜单退出应用。
- 手动输入翻译：在主窗口输入文本后点击"翻译"。
- `Alt+T` 划词翻译：在其他应用中选中文本后按 `Alt+T`，应用会尝试模拟 `Ctrl+C` 读取选中文本并自动翻译。
- `Alt+O` 截图 OCR 翻译：通过 DXGI Desktop Duplication 抓取光标所在显示器整屏帧，在自建 overlay 窗口上鼠标框选区域，经 `Windows.Media.Ocr` 识别后复用翻译链路（Esc / 右键 / 选区过小可取消）。
- OpenAI-compatible 流式翻译 provider：调用兼容 `/v1/chat/completions` 的流式接口。
- Claude / Anthropic 流式翻译 provider：调用 Anthropic Messages API 的 SSE 流式接口，支持 thinking 模式。
- Mock provider：用于无真实 API Key 的本地验证。
- 独立设置页与独立翻译弹窗：主窗口承载设置页（Vue 3 + Tailwind v4 + reka-ui + @lucide/vue），含通用/翻译/快捷键/服务/历史/高级 6 个分类面板，支持多服务实例管理；划词 / OCR 触发时弹出独立翻译弹窗并跟随光标定位，两者互不耦合。
- 流式结果展示：Rust 后端通过 Tauri event 推送翻译状态和增量文本，前端实时渲染。
- 翻译取消与重试：流式翻译过程中可取消，失败或取消后可一键重试。
- OCR 错误指引：截图 OCR 失败（缺语言包 / 识别为空 / 区域过大等）时给出带阶段前缀与可操作指引的错误文案，并隐藏无意义的重试按钮。
- Token 用量展示：流式翻译结束时在译文下方显示 input → output token 数；可在设置页关闭采集。
- 翻译来源徽章：划词翻译显示「来自划词」、OCR 翻译显示「来自 OCR」，手动输入不显示；翻译结束/取消/失败/清空时徽章自动隐藏。

## 使用方式

### 手动翻译

1. 启动应用。
2. 在输入框输入要翻译的文本。
3. 点击"翻译"。

### 划词翻译

1. 在任意支持复制的应用中选中文本。
2. 按 `Alt+T`。
3. Shizi 会尝试读取选中文本并自动翻译。

> 当前划词复制只尽力保护纯文本剪贴板，不保证完整恢复图片、文件、HTML、RTF 等非文本剪贴板格式。

### 截图 OCR 翻译

1. 按 `Alt+O`，整屏冻结为 overlay 画面，鼠标变为十字。
2. 拖动选择要识别的矩形区域。
3. 松开鼠标后，Shizi 调用系统 OCR 识别选区文字并自动翻译。

> Esc、右键或选区过小（<3px）会取消本次截图，不进入翻译。Windows OCR 依赖系统语言包，中英混合需安装对应 OCR 语言包。

## 配置

主窗口为独立设置页（Vue 3 + Tailwind v4 + reka-ui + @lucide/vue + @iconify/vue），当前支持：

- 通用（开机启动/主题/语言/关闭行为/窗口预创建策略/更新）
- 翻译（源语言/目标语言/默认服务实例/复制粘贴行为）
- 快捷键（Alt+T 划词/Alt+O 截图 OCR/取词等 6 项，后端硬编码不可配）
- 服务（内置 15 个渠道 + 自定义渠道，支持多实例、Key 管理、模型拉取、思维链深度、提示词编辑）
- 历史（OCR 翻译历史，含时间戳/源语种/目标语种/原文/译文）
- 高级（日志等级/导出/实验功能/匿名统计/配置导入导出/重置/关于）


> translate / overlay 仍为纯静态 HTML/JS/CSS（`frontend/public/`），overlay 永久不迁。

配置会保存到 Tauri 的应用配置目录下的 `config.json`。

> 注意：当前 MVP 会将 API Key 明文保存到本机配置文件。后续产品化阶段会迁移到 Windows Credential Manager / macOS Keychain / Linux Secret Service 等系统安全存储。

首次没有本地配置文件时，会从以下环境变量读取默认值：

```bash
SHIZI_LLM_PROVIDER=mock | openai-compatible | claude
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
- 快捷键（Alt+T 划词/Alt+O 截图 OCR/取词等 6 项，后端硬编码不可配） 系统安全存储。
- 快捷键自定义、深色模式、部分面板操作（历史/语音输入等）——已在设置页 UI 中以「实现中」标签预留。

截图 OCR 已落地，但存在以下 MVP 已知限制：

- 多显示器下，`Alt+O` 抓帧按光标定位显示器，但 overlay 窗口默认建在主屏，光标在副屏时可能错位。
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
