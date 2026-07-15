# 独立文字识别窗口 · OCR Debug 日志 · 视觉请求默认

- 日期：2026-07-15
- 状态：待用户审查
- 前置：`2026-07-15-multimodal-ocr-runtime-design.md`（视觉 OCR 运行时已接通）

## 1. 背景与问题

截图翻译链路中，启用 **OpenAI 兼容视觉 OCR** 时，识别效果明显弱于同一模型在厂商网页 playground 的表现。排查时发现：

1. **可观测性不足**：Vision 路径 debug 仅有编码宽高与 PNG 字节数；识别正文在 `ocr_translation` 中被 **硬编码** `redact_text(..., "info")`，即便全局日志为 `debug` 也看不到全文。
2. **请求形态可能偏弱**：`image_url` 未设置 `detail`（多数实现默认 `auto`/`low` 会降采样），难以对齐网页「看清小字」的行为。
3. **缺少纯 OCR 产品面**：当前 OCR 仅作为截图翻译的前置步骤，无法单独验证「图→文」质量，也无法对照预览图与元信息。

## 2. 目标与 Done means

### 2.1 目标

在 **不改变 `Alt+S` 截图→OCR→自动翻译** 语义的前提下，交付：

| # | 能力 |
|---|---|
| 1 | **独立「文字识别」窗口**：预览图 + 识别文本 + 元信息面板 |
| 2 | **三种输入**：截图框选、打开文件、剪贴板图片 |
| 3 | **入口**：可配置全局快捷键 + 托盘菜单（默认建议 `Alt+O`） |
| 4 | **Debug/Info 日志增强**，并修复 debug 下全文不可见的脱敏 bug |
| 5 | **Vision 请求默认** `image_url.detail = "high"`（合理默认，非网页参数全集） |

### 2.2 Done means

1. 托盘或快捷键可打开识别窗口；三种输入均可触发当前唯一启用 OCR 引擎。
2. 成功时展示识别全文与元信息（引擎/模型、原图与送模尺寸、PNG 大小、耗时、HTTP status 等适用字段）。
3. 日志级别为 `debug` 时，文件中可见识别全文与 prompt/endpoint 等诊断字段；**永不**写 API Key 明文或 base64 图。
4. Vision 请求体包含 `image_url.detail: "high"`；单元测试锁定。
5. `Alt+S` 截图翻译路径行为与回归一致（仅共享引擎与日志增强，不打开识别窗口）。
6. **本轮不做**「用识别结果一键翻译」。

## 3. 已锁定产品决策

| # | 决策 |
|---|---|
| 1 | 范围：日志 + 独立纯 OCR 窗口 + 必要请求默认（方案包 C） |
| 2 | 正式产品能力，非仅设置页调试工具 |
| 3 | 独立窗口：左/右图文对照 + 底部元信息（线框已确认） |
| 4 | 输入：截图 + 文件 + 剪贴板 |
| 5 | 入口：新快捷键 + 托盘；`Alt+S` 并行保留 |
| 6 | 质量：可诊断日志 + `detail=high` 等合理默认；不开放 temperature/缩放 UI |
| 7 | 元信息进 UI（非仅日志） |
| 8 | 架构：独立 Tauri `ocr` 窗口 + core `recognize` 命令（方案 A） |
| 9 | 一键翻译本轮不做，仅复制文本 |
| 10 | 识别结果不写入翻译历史 SQLite |

## 4. 架构

```
[托盘「文字识别」| 快捷键 ocrShortcut]
        │
        ▼
  open_ocr_window → show/focus ocr 窗口
        │
  ┌─────┴──────────────────────────────┐
  │ 输入                               │
  │  · 截图：DXGI + overlay → crop     │
  │  · 文件：读字节 → CapturedImage    │
  │  · 剪贴板：图像 → CapturedImage    │
  └─────┬──────────────────────────────┘
        ▼
  resolve_ocr_engine(config.ocr_services)
        │
        ├─ WindowsMedia → WindowsOcrEngine
        └─ Vision*      → VisionOcrEngine (detail=high, 日志增强)
        ▼
  recognize → OcrResult { text, lines, engine } + OcrRunMeta
        ▼
  前端渲染：预览 | 文本 | 元信息
  （不调用 TranslationService）
```

分层原则（与项目一致）：

- **核心层**承担识别、编码、HTTP、日志；UI 不直连视觉 API。
- **翻译链路**继续用 `recognize_cropped_for_translation`；共享引擎与日志，不弹 OCR 窗口。
- **OCR 窗口**与 **main 翻译弹窗**、**settings** 解耦，不互相嵌入业务状态机。

### 4.1 模块落点

| 位置 | 职责 |
|---|---|
| `core/ocr/vision_openai.rs` | `detail: high`；请求/响应诊断日志；耗时 |
| `core/ocr/image_encode.rs` | 已有缩放；debug 记录缩放前后（若尚未完整） |
| `core/ocr/` 新类型 | `OcrRunMeta`：`engine`、`model: Option`、`source_width/height`、`sent_width/height`、`png_bytes`、`latency_ms`、`http_status: Option`、`scaled: bool` |
| `core/ocr_translation.rs` | 按当前日志级别 `redact_text`；info 记 meta 摘要 |
| `ui/` OCR 窗口 commands | `open_ocr_window`、`recognize_image`（bytes+format 或内部路径）、截图纯识别编排（与翻译入口分叉） |
| `app/shortcuts.rs` + tray | 注册 OCR 快捷键；托盘菜单项「文字识别」 |
| `core/config` | 快捷键字段与设置页全局快捷键模块对齐，默认 `Alt+O`；冲突进 `shortcut_conflicts` |
| `frontend` 新入口 | `ocr.html` + Vue 页（与 settings 同 Vite 工程；overlay 仍独立静态页） |
| `tauri.conf.json` / capabilities | 新 `ocr` 窗口；所需 core 权限与快捷键权限 |
| i18n | 窗口标题、按钮、空态/错误文案 |

## 5. 独立识别窗口

### 5.1 布局

- **工具栏**：截图 | 打开文件 | 从剪贴板；右侧显示当前引擎摘要（type + model，Windows 无 model 则省略）。
- **主区双栏**：左图像预览；右识别文本 + 复制按钮。
- **底部元信息**：原图像素、送模像素、PNG 字节、耗时、HTTP status（适用时）、引擎 type、model。
- 窗口为应用生命周期内单例：`CloseRequested` → `hide()`（与 main/settings 托盘驻留模型一致），再次打开 `show` + `focus`。

### 5.2 输入

| 来源 | 行为 |
|---|---|
| 截图 | 复用 DXGI + overlay；提交区域后 crop → recognize；**不**调用翻译入口 |
| 文件 | 系统选图（png/jpg/webp/bmp 等实现时支持的集合）；解码为 `CapturedImage` |
| 剪贴板 | 读取位图；无图时状态提示，不记 error 风暴 |

识别进行中按钮 disabled 或显示 loading；取消截图不视为失败。

### 5.3 输出与历史

- 成功：文本区全文；可一键复制。
- 失败：文本区或状态区显示已有 OCR 错误映射文案；保留已采集到的 meta。
- **不写** `HistoryStore`；**不做**「翻译此文本」按钮（本轮）。

## 6. 日志与脱敏

### 6.1 字段分级

| 级别 | 字段 |
|---|---|
| `info` | 引擎 type、model、原图尺寸、送模尺寸、png 字节数、耗时 ms、HTTP status、识别文本**摘要**（`redact_text` info 规则） |
| `debug` | endpoint（无 query 密钥）、system/user prompt 全文、裁剪物理矩形（截图路径）、是否发生缩放、识别**全文**、错误响应 body 截断摘要、`finish_reason` / usage（若响应含） |
| 永不 | API Key 明文、完整 base64、data URL 载荷 |

### 6.2 必修复

- `recognize_cropped_for_translation`（及新识别路径）**禁止**写死 `"info"` 调用 `redact_text`。
- 按运行时有效日志级别选择摘要/全文：以 `log::max_level()` 是否包含 `Level::Debug` 为准（`save_app_config` 已同步 `log::set_max_level`，与配置 `logLevel` 一致）。

### 6.3 Windows 引擎

同样记录引擎名、尺寸、耗时、文本（debug 全文）；无 HTTP 则省略相关字段。

## 7. 视觉请求默认（质量）

| 项 | 决策 |
|---|---|
| `image_url.detail` | 固定 `"high"`；单测断言 |
| 最长边缩放 | 保持 `VISION_MAX_LONG_EDGE = 2048`；debug 记缩放前后 |
| `max_tokens` | 保持 2048 |
| 流式 | 仍非流式 |
| Prompt | 配置 `ocrPrompt`，空则 `DEFAULT_OCR_PROMPT` |
| UI 可配 detail/温度/缩放 | **本轮不做** |

兼容端点忽略未知字段时 `detail` 无害；若某厂商因 `detail` 报错，实现阶段记 warn 并评估剥离（默认仍发 `high`）。

## 8. 配置与入口

- 新增快捷键配置项（名称实现时与设置页「全局快捷键」模块对齐），默认 `Alt+O`。
- 启动注册 best-effort：冲突写入 `shortcut_conflicts`，与现有快捷键策略一致。
- 托盘增加「文字识别」。
- 设置页快捷键模块展示/编辑该项；**保存仍 all-or-nothing + 失败回滚**（沿用现有）。

## 9. 错误处理

- 复用 `OcrError` → 用户文案映射（无引擎、鉴权、空结果、不支持协议、HTTP 等）。
- 用户取消截图 / 剪贴板无图 / 取消文件对话框：UI 轻提示，不触发全局失败弹窗。
- 识别失败不影响翻译弹窗状态。

## 10. 范围外（YAGNI）

- Claude 视觉协议接通
- 多 OCR 并行对比
- 识别历史持久化
- 识别+翻译合一 API
- 开放 detail/温度/max_tokens/缩放配置 UI
- 修改 `Alt+S` 为纯识别或双模式
- 一键「翻译此文本」
- 专用云 OCR（百度/腾讯等）

## 11. 测试计划

| 层 | 内容 |
|---|---|
| Rust 单测 | `build_request_body` 含 `detail: "high"`；`redact_text` 随 level；meta 组装/序列化纯函数 |
| Rust 单测 | 配置快捷键默认与 normalized（若触及） |
| 前端 | 窗口状态 idle/loading/success/error；类型检查 |
| 手动 | 三种输入；Windows 与 Vision；logLevel=debug 可见全文与 prompt；`Alt+S` 仍自动翻译 |

## 12. 风险与注意

- 部分「OpenAI 兼容」端点对 `detail` 支持不一：默认 high + 日志可诊断即可。
- 高分辨率 + `detail=high` 可能增加费用与延迟：属刻意权衡，以准确率优先。
- 剪贴板/文件解码格式差异需在实现中明确错误信息。
- 截图路径需严格分叉：纯识别 vs 翻译，避免误触 `start_translation`。

## 13. 成功标准（验收）

1. 新快捷键与托盘可打开识别窗口。  
2. 截图/文件/剪贴板均可完成识别并显示文本与元信息。  
3. debug 日志可支撑「与网页对比」所需关键信息（模型、尺寸、prompt、全文、耗时）。  
4. Vision 请求带 `detail: high`。  
5. `Alt+S` 与翻译批次无回归。  
6. 无 API Key / 图像 base64 泄漏进日志。  
