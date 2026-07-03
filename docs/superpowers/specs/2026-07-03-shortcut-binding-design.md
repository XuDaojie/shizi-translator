# 快捷键绑定功能设计

## 背景

当前设置页「全局快捷键」面板已经有录入控件和 6 条绑定项，但真实后端只硬编码注册 `Alt+T` 划词翻译与 `Alt+O` 截图翻译。用户在设置页修改快捷键后，配置不会进入 Tauri 后端，也不会影响全局快捷键注册，导致该模块处于“有界面、未适配功能”的状态。

本轮目标是把快捷键设置接入真实全局快捷键系统。优先保证划词翻译与截图翻译可改、可保存、可立即生效；低成本功能一并完成；取词翻译暂不实现触发行为。

## 目标范围

### 必须实现

- `translate-selection`：绑定到划词翻译，复用现有选中文本复制与翻译链路。
- `translate-screenshot`：绑定到截图 OCR 翻译，复用现有 overlay + OCR + 翻译链路。
- 设置页保存后，新的快捷键无需重启即可生效。
- 快捷键为空表示禁用该动作。
- 注册失败时不保存配置，并在设置页对应行展示错误。

### 同轮完成的低成本动作

- `translate-clipboard`：读取当前剪贴板文本并复用翻译弹窗链路。
- `show-window`：显示并聚焦主设置窗口。
- `open-settings`：显示并聚焦主设置窗口；现阶段与 `show-window` 复用同一行为。

### 明确不做

- `word-lookup` 取词翻译不注册全局快捷键，只保留配置保存能力。它需要鼠标悬停取词、目标窗口文本探测等独立链路，不适合塞进本次快捷键绑定适配。
- 不新增快捷键分组、冲突扫描服务、快捷键 profile 或导入导出。
- 不参考或翻译 `pot-desktop/` 代码实现。

## 数据模型

后端 `AppConfig` 增加 `shortcuts` 字段，采用与前端 `ShortcutBinding.id` 对齐的稳定 id 到快捷键字符串的映射。

默认值：

| id | 默认快捷键 | 行为 |
| --- | --- | --- |
| `translate-selection` | `Alt+T` | 划词翻译 |
| `translate-clipboard` | `Ctrl+Shift+C` | 剪贴板翻译 |
| `translate-screenshot` | `Alt+O` | 截图翻译 |
| `word-lookup` | 空 | 仅保存，不注册 |
| `show-window` | `Ctrl+Shift+Space` | 显示主窗口 |
| `open-settings` | `Ctrl+,` | 打开设置 |

归一化规则：

- 缺失字段补默认值。
- 空字符串保留为空，表示禁用。
- 前后端字段保持 camelCase JSON；Rust 内部保持 snake_case。

## 后端设计

`src-tauri/src/app/shortcuts.rs` 作为唯一全局快捷键注册与分发入口：

- 启动时从 `ConfigStore` 读取 `AppConfig.shortcuts` 注册非空、已实现动作。
- 保存配置时先验证并尝试注册新快捷键，成功后再写入 `ConfigStore`。
- 重注册采用插件现有 `unregister_all()` + `register()` 能力，保持实现简单。
- handler 收到触发事件后，根据当前配置把 `Shortcut` 反查为动作 id，再分发到对应行为。

动作分发：

- `translate-selection` → 现有 `handle_selection_translate`。
- `translate-screenshot` → 现有 `start_translation_from_ocr`。
- `translate-clipboard` → 读取剪贴板文本，设置 pending source text，显示弹窗并调用 `start_translation_from_input(ManualText)`。
- `show-window` / `open-settings` → 复用现有主窗口显示逻辑。
- `word-lookup` → 不注册，因此不会触发。

错误处理：

- 单条快捷键解析失败：返回该绑定 id 的错误。
- 系统占用或注册失败：返回该绑定 id 的错误。
- 同一配置内重复快捷键：前端先标红，后端也拒绝保存。
- 触发时读取不到剪贴板文本：通过翻译弹窗展示不可重试错误。

## 前端设计

`ShortcutPanel.vue` 移除 `translate-selection` / `translate-screenshot` 的只读限制：

- 所有绑定都可编辑和清空。
- 已实现动作不再显示“开发中”标签。
- `word-lookup` 行保留“规划中”状态，说明当前仅保存绑定。
- 本地录入后继续通过现有设置页保存按钮提交。

`frontend/src/lib/config.ts` 在 `projectToAppConfig()` 中把 `state.shortcut.bindings` 投影到后端 `shortcuts` 字段。`frontend/src/types/config.ts` 与 Rust `AppConfig` 同步新增字段。

快捷键格式沿用当前 `ShortcutRecorder` 输出，例如 `Alt+T`、`Ctrl+Shift+C`、`Ctrl+,`。实现阶段只做必要规范化，不新增复杂格式转换器。

## 验收标准

- 修改划词翻译快捷键，保存后旧快捷键失效，新快捷键触发划词翻译。
- 修改截图翻译快捷键，保存后旧快捷键失效，新快捷键触发截图 OCR 翻译。
- 清空划词或截图快捷键并保存后，对应动作不再触发。
- 剪贴板翻译快捷键可触发当前剪贴板文本翻译；剪贴板为空时显示错误。
- 显示主窗口 / 打开设置快捷键可唤起主窗口。
- 重复快捷键或系统占用快捷键保存失败，并在对应行显示错误。
- 重启应用后仍使用保存后的快捷键。

## 测试与验证

- Rust 单元测试：
  - 默认快捷键归一化。
  - 空快捷键禁用注册。
  - 重复快捷键被拒绝。
  - `Shortcut` 反查动作不依赖字符串大小写展示。
- 前端单元测试：
  - `projectToAppConfig()` 输出 `shortcuts`。
  - 重复绑定校验返回对应错误。
- 构建验证：
  - `cd src-tauri && cargo test`
  - `npm run typecheck`
  - `npm run test`
- 手动验证：
  - Windows 上运行 `npm run tauri dev`，逐项验证验收标准。

## 文档同步

编码完成后需要同步：

- `AGENTS.md` 与 `CLAUDE.md` 的全局快捷键说明。
- `docs/architecture/screenshot-ocr-architecture.md` 的快捷键配置现状。
- `docs/roadmap/progressive-development-plan.md` 的快捷键体验打磨进度。
