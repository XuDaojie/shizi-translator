# 文字识别窗：PDF 首頁 OCR + 会话级渠道切换

- 日期：2026-07-16
- 状态：已实现
- 前置：`2026-07-15-multimodal-ocr-runtime-design.md`、`2026-07-15-services-drag-detail-ocr-design.md`、OCR 独立窗口已接通

## 1. 背景与目标

文字识别独立窗口已支持：截图框选、打开图片文件、剪贴板位图、重新识别；OCR 引擎由设置页 `ocrServices[]` 中**唯一启用**实例驱动。

用户希望：

1. **打开文件支持 PDF**：选 PDF 后做文字识别（本轮仅第 1 页）。
2. **剪贴板**：维持现状（仅系统位图/截图类图片，不扩展 PDF/文件路径）。
3. **识别页临时切换 OCR 渠道**：只影响本窗口当前会话；不改设置页启用状态；窗口重开 / 应用重启后仍回落到设置页启用的引擎。

**Done means**

1. 文件选择器可选 PDF；成功打开后渲染 **第 1 页** 为位图，走现有 OCR 引擎（Windows 媒体 / OpenAI 兼容视觉均可），预览与正文可用。
2. 多页 PDF 仅处理第 1 页；可提示「已识别第 1 页（共 N 页）」（实现时有页数则展示，无则省略）。
3. OCR 窗顶栏可切换临时渠道：列表 = 配置中**全部** `ocrServices` 实例；默认 = 当前 `enabled` 那一项。
4. 临时选择不写 `config.json`，不改变设置页互斥启用状态。
5. 本窗内截图 / 打开文件 / 剪贴板 / 重新识别均使用当前临时 `serviceId`。
6. **不**影响 `Alt+S` 截图翻译链路（仍只用设置页启用引擎）。
7. 剪贴板无功能扩展（仅现有位图路径）。

## 2. 范围

### 2.1 范围内

| 项 | 内容 |
|---|---|
| PDF 打开 | 文件过滤器增加 `pdf`；按扩展名/嗅探分支；第 1 页栅格化 → `CapturedImage` |
| PDF 渲染 | Windows 优先：`Windows.Data.Pdf`（或等价 WinRT）渲染第 1 页为 RGBA/BGRA 位图，**不**引入外部 pdfium 动态库（除非实现期证明 WinRT 不可用再换） |
| 临时渠道 UI | OCR 窗 session 状态 `selectedOcrServiceId`；下拉展示全部已配置实例 |
| 临时渠道后端 | 识别命令接受可选 `service_id`；`resolve_ocr_engine` 支持按 id 解析（忽略 enabled） |
| 截图纯识别 | `start_ocr_capture` / `submit_capture_region(RecognizeOnly)` 使用会话槽中的临时 id |
| 错误与文案 | 损坏 PDF、空文档、渲染失败、未知 id、视觉缺 Key 等中文错误 |
| 测试 | 栅格化 / resolve_by_id / 不写配置启用位 的单测；必要前端类型 |

### 2.2 范围外（YAGNI）

- 剪贴板 PDF / 文件路径 / 多格式增强
- 多页 PDF 全量识别、页码选择 UI、缩略图浏览器
- 设置页启用互斥规则变更
- 翻译弹窗 / `Alt+S` 翻译路径的临时渠道
- 将 PDF 原始字节直接作为 vision API 的 file/document 输入（本轮统一栅格化）
- 非 Windows 平台 PDF（当前产品 Windows-first；unsupported 返回明确错误即可）

## 3. 已锁定产品决策

| # | 决策 |
|---|---|
| 1 | 架构路线：**薄扩展现有识别链路**（方案 A），不新建文档子系统 |
| 2 | PDF：**仅第 1 页** |
| 3 | 临时渠道列表：**全部已配置** `ocrServices`（含未启用）；缺 Key 仍可列，识别时报错 |
| 4 | 切换渠道：**只影响后续识别**，不自动重跑 |
| 5 | 作用域：**仅文字识别窗口**；设置页 `enabled` 不变；重启/重开回落启用项 |
| 6 | 剪贴板：**维持现状**（仅位图） |
| 7 | 视觉 / Windows 引擎均只接收栅格图；PDF 先转图再 OCR |

## 4. 架构

```
[OCR 窗 UI]
  selectedOcrServiceId (session, 不持久化)
       │
       ├─ pick_and_recognize_image(service_id?)
       │     ├─ image/* → load_image_file_bytes
       │     └─ .pdf    → render_pdf_first_page → CapturedImage
       │              └─ recognize_image_full(..., services, override_id)
       │
       ├─ recognize_clipboard_image(service_id?)  // 仍仅位图
       ├─ rerecognize_last_image(service_id?)
       └─ start_ocr_capture(service_id?)
              → AppState.ocr_session_service_id (仅 RecognizeOnly)
              → submit 时 resolve + OCR

resolve 规则（OCR 窗路径）：
  if service_id Some(id) → 在 ocr_services 找 id，按 type 建引擎（不要求 enabled）
  else → 现有「唯一 enabled」规则（设置页默认 / 翻译路径）

设置页 / Alt+S 翻译：
  不读 ocr_session_service_id；仍只用 enabled 实例
```

分层：

- **PDF 栅格化**：平台层（`platform/windows`）或 `core/ocr` 旁的小模块，输出 `CapturedImage`，不调用网络。
- **引擎选择**：扩展 `resolve_ocr_engine`（或新增 `resolve_ocr_engine_for_id`），UI/command 传入可选 id。
- **会话覆盖**：仅 OCR 窗相关 command + `CapturePurpose::RecognizeOnly` 路径；**禁止**泄漏到 `CapturePurpose::Translate`。

## 5. PDF 处理

### 5.1 打开文件

扩展 `pick_and_recognize_image`（可改名语义为「打开并识别」，command 名可保留以减 IPC 抖动）：

- 过滤器：`Images`（png/jpg/jpeg/webp/bmp）+ `PDF`（pdf），或合并为「图片与 PDF」。
- 读入字节后：
  - 扩展名为 `.pdf`（大小写不敏感）或 PDF 魔数 `%PDF` → PDF 路径；
  - 否则走现有 `load_image_file_bytes`。

### 5.2 渲染第 1 页

推荐实现（Windows）：

1. 用 WinRT `PdfDocument::LoadFromStreamAsync`（或同步包装）加载字节流。
2. `page_count == 0` → 明确错误。
3. 取 index `0` 页，渲染到合适 DPI（建议逻辑约 150–200 DPI 或固定长边上限，与 vision 编码 2048 长边策略协调，避免无意义超大图）。
4. 输出 `CapturedImage`（RGBA8 或 BGRA8，与现有编码器兼容）。
5. 可选：返回 `page_count` 供 UI/meta 展示「第 1 页 / 共 N 页」。

失败映射：

| 情况 | 用户可见错误（示意） |
|---|---|
| 非 PDF / 损坏 | 无法打开 PDF 文件 |
| 0 页 | PDF 中没有可识别的页面 |
| 渲染失败 | PDF 页面渲染失败 |
| 非 Windows | 当前平台暂不支持 PDF 识别 |

### 5.3 与 OCR 衔接

- 渲染结果写入 `last_ocr_image`（与图片一致），支持「重新识别」。
- 预览仍用现有 PNG base64 路径。
- **不**把整份 PDF 存进 `last_ocr_image`。

## 6. 会话级渠道切换

### 6.1 前端

- 挂载时 `get_app_config`：
  - 列表 = `ocrServices`（全部实例，展示 `name` / `service_type` / `model` 摘要）；
  - 默认 `selectedOcrServiceId` = 第一个 `enabled` 的 id；若无 enabled（异常配置）则取列表第一项并依赖后端错误。
- 顶栏控件：下拉（或 combobox）切换 `selectedOcrServiceId`；**不** invoke 保存配置。
- 所有识别 invoke 携带当前 `selectedOcrServiceId`。
- 监听 `app-config:changed`（若 OCR 窗已开）：刷新列表；若当前 id 已不存在则回落到新的 enabled 默认；**不**在配置变更时自动重跑 OCR。

### 6.2 后端 API 形状（建议）

| Command | 变更 |
|---|---|
| `pick_and_recognize_image` | 增加可选 `service_id: Option<String>` |
| `recognize_clipboard_image` | 同上 |
| `rerecognize_last_image` | 同上 |
| `start_ocr_capture` | 同上；写入 `AppState` 会话槽供 submit 使用 |
| （可选）`list_ocr_services_for_window` | 若不想前端解析完整 config，可返回 `{ id, name, serviceType, model, enabled }[]`；**优先复用 `get_app_config`** 以减少 command 数 |

`submit_capture_region` 在 `RecognizeOnly` 分支：

- 读取会话槽 `ocr_session_service_id`；
- `resolve` 时使用该 id（若有）；
- finish/cancel 后 **清除** 会话槽（或下次 start 覆盖），避免脏状态。

### 6.3 Resolve 规则

```
resolve_for_ocr(services, override_id: Option<&str>) -> Result<ResolvedOcrEngine, OcrError>

override_id = Some(id):
  find services by id
  missing → OcrError（未知服务）
  map_service(instance)  // 与现网 type 映射相同；视觉缺 Key → Auth
  // 不检查 enabled

override_id = None:
  现有 resolve_ocr_engine(services)  // 仅 enabled
```

Claude 视觉等「不可用协议」：配置里可列出；选择后识别返回现有 `UnsupportedProtocol` 友好文案。

### 6.4 与设置页隔离

- 任何临时切换 **不得** 调用 `save_app_config` 改 `enabled`。
- 设置页仍保证全局「至多一个 enabled」；OCR 窗临时用未启用实例时，设置页 UI 不必高亮为启用。

## 7. 错误处理

| 场景 | 行为 |
|---|---|
| 用户取消文件框 | `Ok(None)`，UI 回 success/idle（现有） |
| PDF 渲染失败 | 错误条展示，不写 last_ocr_image |
| 临时 id 不存在 | 错误条：渠道已不存在，请重新选择 |
| 视觉缺 Key | 现有 Auth 友好文案 |
| 识别中切换渠道 | 允许改下拉，不取消进行中的请求；完成结果仍属于旧请求（可用 generation/忽略乱序，MVP 可简单：loading 时 disable 下拉 **或** 允许切换但不关联进行中请求——推荐 **loading 时禁用下拉** 减少竞态） |

**锁定**：识别 loading 期间禁用渠道下拉与入口按钮（与现有按钮 disable 一致）。

## 8. 测试计划

| 类型 | 用例 |
|---|---|
| 单元 | `resolve_for_ocr`：by id 忽略 enabled；缺 id 错误；None 走 enabled |
| 单元 | PDF 第 1 页：用最小合法 PDF fixture（或 mock 渲染接口）得到非空位图尺寸 |
| 单元 | 图片路径回归：`load_image_file_bytes` 不变 |
| 单元 | 会话槽：set/clear；Translate 路径不读槽（若可测） |
| 集成/手动 | 打开多页 PDF 仅见第 1 页内容；切换未启用视觉渠道成功识别且设置页 enabled 不变；重启后下拉回到启用项 |
| 前端 | 类型与 invoke 参数；可选 vitest 纯函数：默认 id 选取 |

## 9. 实现落点（建议）

| 位置 | 职责 |
|---|---|
| `platform/windows/pdf.rs`（新）或 `core/ocr/pdf_render.rs` | PDF 第 1 页 → `CapturedImage` |
| `core/ocr/resolve.rs` | `resolve_for_ocr` / by id |
| `ui/ocr_window.rs` | command 参数、文件分支、传 id |
| `app/state.rs` | `ocr_session_service_id` 槽 + clear |
| `ui/overlay.rs` | RecognizeOnly 读槽 resolve |
| `frontend/src/ocr/OcrWindow.vue` | 渠道下拉 + 传 id |
| `Cargo.toml` / windows features | 按需增加 `Data_Pdf` 等 WinRT feature |

## 10. 风险与缓解

| 风险 | 缓解 |
|---|---|
| WinRT PDF 渲染 DPI/色彩与 OCR 效果 | 固定渲染参数 + 复用 vision 长边缩放；手动样例验收 |
| 大页渲染内存 | 渲染时限制目标像素上限 |
| 会话槽泄漏到翻译截图 | purpose 分叉 + 单测/代码审 |
| 配置变更删除当前 id | 监听 config 变更后回落 enabled |

## 11. 剪贴板结论（记录）

- 现状：`read_clipboard_image`（arboard 位图）+ UI「从剪贴板」**已可用**。
- 本轮：**不**扩展 PDF/文件/HTML 剪贴板。
- 用户确认：保持现状，剪贴板侧只需支持截图类位图场景。

## 12. 实现顺序建议

1. `resolve_for_ocr(override_id)` + 单测  
2. OCR command 透传 `service_id` + AppState 会话槽 + RecognizeOnly 读取  
3. 前端渠道下拉 + 传参  
4. PDF 第 1 页渲染 + 文件选择器扩展 + fixture 测试  
5. 文案 / meta 页数提示 / 手动验收  

---

**规格自检（编写时）**

- [x] 无 TODO/待定占位  
- [x] 与「仅 OCR 窗 / 不改设置启用」一致  
- [x] 范围可单计划覆盖  
- [x] PDF 多页、剪贴板、切换行为已显式锁定  
