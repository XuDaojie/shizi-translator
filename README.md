# Shizi

基于大语言模型的 Windows 桌面翻译助手，灵感来自 macOS 的 Bob。

## 当前能力

当前 MVP 基于 Tauri 2 + Rust 后端 + 原生静态 Web 前端实现，已经具备：

- 托盘常驻：关闭窗口会隐藏到托盘，通过托盘菜单退出应用。
- 手动输入翻译：在主窗口输入文本后点击“翻译”。
- `Alt+T` 划词翻译：在其他应用中选中文本后按 `Alt+T`，应用会尝试模拟 `Ctrl+C` 读取选中文本并自动翻译。
- OpenAI-compatible 流式翻译 provider：调用兼容 `/v1/chat/completions` 的流式接口。
- Mock provider：用于无真实 API Key 的本地验证。
- 内嵌设置面板：配置目标语言、API Key、Base URL、模型名和超时时间。
- 流式结果展示：Rust 后端通过 Tauri event 推送翻译状态和增量文本，前端实时渲染。

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

## 配置

主窗口内嵌“设置”面板，当前支持：

- 目标语言
- API Key
- Base URL
- Model
- Timeout 秒

配置会保存到 Tauri 的应用配置目录下的 `config.json`。

> 注意：当前 MVP 会将 API Key 明文保存到本机配置文件。后续产品化阶段会迁移到 Windows Credential Manager / macOS Keychain / Linux Secret Service 等系统安全存储。

首次没有本地配置文件时，会从以下环境变量读取默认值：

```bash
SHIZI_LLM_PROVIDER=mock | openai-compatible
SHIZI_TARGET_LANG=中文
SHIZI_OPENAI_API_KEY=...
SHIZI_OPENAI_BASE_URL=https://api.openai.com/v1
SHIZI_OPENAI_MODEL=gpt-4o-mini
SHIZI_OPENAI_TIMEOUT_SECS=60
```

本地 mock 模式示例：

```bash
SHIZI_LLM_PROVIDER=mock npm run tauri dev
```

## 当前限制

以下能力尚未实现：

- Anthropic / Claude 专用 provider。
- OCR / 截图翻译。
- Slint 原生高性能翻译弹窗。
- 独立设置窗口；当前设置仍内嵌在主窗口。
- API Key 系统安全存储。
- 翻译取消、重试按钮、历史记录、快捷键自定义。

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
