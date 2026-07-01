# Shizi

基于大语言模型的 Windows 桌面翻译助手，灵感来自 macOS 的 Bob。

## 当前能力

当前 MVP 基于 Tauri 2 + Rust 后端 + 原生静态 Web 前端实现，已经具备：

- 托盘常驻：关闭窗口会隐藏到托盘，通过托盘菜单退出应用。
- 手动输入翻译：在主窗口输入文本后点击“翻译”。
- `Alt+T` 划词翻译：在其他应用中选中文本后按 `Alt+T`，应用会尝试模拟 `Ctrl+C` 读取选中文本并自动翻译。
- `Alt+O` 截图 OCR 翻译：通过 DXGI Desktop Duplication 抓取光标所在显示器整屏帧，在自建 overlay 窗口上鼠标框选区域，经 `Windows.Media.Ocr` 识别后复用翻译链路（Esc / 右键 / 选区过小可取消）。
- OpenAI-compatible 流式翻译 provider：调用兼容 `/v1/chat/completions` 的流式接口。
- Claude / Anthropic 流式翻译 provider：调用 Anthropic Messages API 的 SSE 流式接口，支持 thinking 模式。
- Mock provider：用于无真实 API Key 的本地验证。
- 独立设置页与独立翻译弹窗：主窗口承载设置页，划词 / OCR 触发时弹出独立翻译弹窗并跟随光标定位，两者互不耦合。
- 流式结果展示：Rust 后端通过 Tauri event 推送翻译状态和增量文本，前端实时渲染。
- 翻译取消与重试：流式翻译过程中可取消，失败或取消后可一键重试。
- Token 用量展示：流式翻译结束时在译文下方显示 input → output token 数；可在设置页关闭采集。
- 翻译来源徽章：划词翻译显示「来自划词」、OCR 翻译显示「来自 OCR」，手动输入不显示；翻译结束/取消/失败/清空时徽章自动隐藏。

## 使用方式

### 手动翻译

1. 启动应用。
2. 在输入框输入要翻译的文本。
3. 点击“翻译”。

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

主窗口为独立设置页，当前支持：

- 目标语言
- API Key
- Base URL
- Model
- Timeout 秒

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
- API Key 系统安全存储。
- 翻译历史记录、快捷键自定义。

截图 OCR 已落地，但存在以下 MVP 已知限制：

- 多显示器下，`Alt+O` 抓帧按光标定位显示器，但 overlay 窗口默认建在主屏，光标在副屏时可能错位。
- 缩放比例取主窗口近似目标显示器，混合 DPI 多屏下框选坐标可能不准。
- 锁屏 / 屏保 / 安全桌面 / 远程会话下 DXGI 抓帧可能失败。

## 命令

### 运行（开发模式）

```bash
npm run tauri dev
```

### 编译（debug）

```bash
cd src-tauri && cargo build
```

### 测试（Rust）

```bash
cd src-tauri && cargo test
```

### 前端语法检查

```bash
node --check frontend/main.js
```

### 打包（release）

```bash
cd src-tauri && cargo build --release
# release exe: src-tauri/target/release/shizi.exe
```

### 生成安装包（MSI/NSIS）

```bash
npm run tauri build
# 安装包: src-tauri/target/release/bundle/msi/ 或 bundle/nsis/
```

### 调试

```bash
npm run tauri dev
# 或直接运行 release exe:
./src-tauri/target/release/shizi.exe
```

### 清理编译缓存

```bash
cd src-tauri && cargo clean
# 删除整个 target 目录（可节省数 GB 空间）
```

> `npx tauri dev` 也可代替 `npm run tauri dev` 执行。
