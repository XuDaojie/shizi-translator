# 托盘原生菜单样式对齐 设计规格

- 日期：2026-07-16
- 状态：已实现（编码执行于 `feat/tray-native-menu`；本轮托盘菜单文案为中文产品常量，未扩 8 语 `tray.*` locale，见 plan 澄清）
- 关联：
  - OpenDesign 原型：`C:\Users\xdj\IdeaProjects\LLM\OpenDesignProjects\shizi`（`src/tray/TrayNativeMenu.vue` / `TrayMenuPage.vue`）
  - [application-internationalization-design](./2026-07-11-application-internationalization-design.md)（托盘文案与 `TrayI18nHandles`）
  - [shortcut-stability-icon-refresh-design](./2026-07-04-shortcut-stability-icon-refresh-design.md) / 当前 `AppConfig.shortcuts` 与 `shortcuts.rs` 动作分发

## 1. 目的

按高保真原型的 **A · 系统原生菜单** 约束，重排 shizi 托盘右键菜单的信息架构：项文案、分组分隔线、右侧加速键展示，并补齐「截图翻译」入口。外观颜色/字体/圆角完全交给 OS，不引入 WebView 品牌小窗。

## 2. 范围

### 范围内

- 继续使用 Tauri `TrayIcon` + 系统 `Menu` / `MenuItem` / 分隔项（`PredefinedMenuItem::separator` 或等价 API）
- 菜单结构对齐原型分组语义，并保留产品已有「文字识别」
- 菜单项文案走 i18n（`tray.*`），界面语言切换时原位更新
- 加速键字符串读自 `AppConfig.shortcuts`，与设置页 / 全局快捷键同源；空绑定不显示加速键
- 新增托盘「截图翻译」动作，复用现有 `ShortcutAction::OcrTranslate` 同一执行路径
- 配置保存后刷新菜单加速键展示（与快捷键重绑同一保存路径）
- 单元测试：从 shortcuts map 生成 accelerator 选项的纯函数
- 文档：本 spec；实现计划另文

### 范围外（YAGNI）

- **B · WebView 无边框小窗**品牌菜单（图标、kbd 胶囊、footer 状态条）
- 左键 WebView / 右键原生双轨分流
- 托盘菜单项上**额外注册**全局快捷键（禁止与 `tauri-plugin-global-shortcut` 双绑；加速键仅展示，触发仍靠已注册全局快捷键或菜单点击）
- 退出项绑定 `Ctrl+Q`（产品未配置该快捷键，本轮不新增）
- 自定义菜单绘制、自绘 hover 蓝条、模拟 Win32 控件外观
- 修改托盘图标资源、双击托盘行为
- 参考或翻译 `pot-desktop/` 实现

## 3. 背景与现状

### 3.1 原型（A · 原生菜单）

`TrayNativeMenu.vue` 约束：

- 纯文本项 + 右侧加速键
- 分隔线分组
- 无自定义图标色、圆角卡片、状态点、底部状态条
- 颜色 / 字体完全交给 OS

原型示意项：划词翻译、截图翻译、偏好设置、退出 shizi（演示加速键为 `Ctrl+Shift+…`，**产品以真实配置为准**）。

### 3.2 当前实现

`src-tauri/src/app/tray.rs`：

| 项 | 文案 | 动作 | 加速键 |
|---|---|---|---|
| translate | 翻译 | `show_translation_popup` | 无 |
| ocr | 文字识别 | `show_ocr_window` | 无 |
| settings | 设置 | `show_settings_window` | 无 |
| quit | 退出 | `app.exit(0)` | 无 |

扁平列表、无分隔线；i18n 仅覆盖 `tray.translate` / `tray.settings` / `tray.quit` / `tray.tooltip`（OCR 项固定中文）。

产品默认快捷键（`default_shortcuts`）：

- `translate-selection` → `Alt+D`
- `translate-screenshot` → `Alt+S`
- `ocr-recognize` → `Alt+O`
- `open-settings` → `Ctrl+,`

## 4. 方案对比与定稿

| 方案 | 内容 | 结论 |
|---|---|---|
| 1 严格原型 | 划词 / 截图 / 设置 / 退出，去掉文字识别 | 否：丢失已有 OCR 入口 |
| 2 最小改动 | 仅加分隔与加速键，保留现四项与旧文案 | 否：缺截图翻译，文案不对齐 |
| **3 原型骨架 + 产品能力** | 划词 / 截图 / 文字识别 + 分隔 + 偏好设置 + 分隔 + 退出 | **已选** |

## 5. 信息架构（定稿）

菜单从上到下：

```
划词翻译          <accel: translate-selection>
截图翻译          <accel: translate-screenshot>
文字识别          <accel: ocr-recognize>
────────
偏好设置          <accel: open-settings>
────────
退出 shizi        （无加速键）
```

### 5.1 菜单 id 与动作

| menu id | i18n key（文案） | 动作 | 加速键配置 id |
|---|---|---|---|
| `selection` | `tray.selection` | 现有打开翻译弹窗路径（等同原 `translate`） | `translate-selection` |
| `screenshot` | `tray.screenshot` | 与全局快捷键 `OcrTranslate` 相同（截图 OCR 后进翻译链路） | `translate-screenshot` |
| `ocr` | `tray.ocr` | 现有 `show_ocr_window`（不自动翻译） | `ocr-recognize` |
| `settings` | `tray.settings` | 现有 `show_settings_window` | `open-settings` |
| `quit` | `tray.quit` | `app.exit(0)` | — |

分隔项无 id、不参与 i18n 更新。

### 5.2 文案（zh-CN 事实来源）

| key | zh-CN | 说明 |
|---|---|---|
| `tray.selection` | 划词翻译 | 替换旧 `tray.translate`「翻译」 |
| `tray.screenshot` | 截图翻译 | 新增 |
| `tray.ocr` | 文字识别 | 新增（原硬编码中文纳入 i18n） |
| `tray.settings` | 偏好设置 | 由「设置」改为原型用词 |
| `tray.quit` | 退出 shizi | 由「退出」改为原型用词 |
| `tray.tooltip` | （保持现有产品文案策略） | 本轮可不改语义，仅随语言包维护 |

**Key 迁移**：删除或停止使用 `tray.translate`。内置 8 语 locale 全部改为新 key；用户语言包若仍带 `tray.translate`，解析层按现有「user → 内置 → zh-CN」回退，缺失 key 由内置补齐（不要求用户包立即升级）。

英文示例（实现时可微调，需 8 语齐全）：

- `tray.selection` → Selection Translate
- `tray.screenshot` → Screenshot Translate
- `tray.ocr` → Text Recognition
- `tray.settings` → Preferences
- `tray.quit` → Quit shizi

## 6. 加速键

### 6.1 展示规则

- 从当前内存中的 `AppConfig.shortcuts` 读取对应 id
- `trim` 后为空 → `MenuItem` accelerator 为 `None`（不显示）
- 非空 → 将配置字符串原样交给 Tauri `MenuItem` 的 accelerator 参数（与设置页展示同源，如 `Alt+D`、`Ctrl+,`）
- 解析失败：该项 accelerator 置 `None` 并 `log::warn`，菜单仍可点，不阻断托盘 setup

### 6.2 禁止双绑

- 全局快捷键仍仅由 `register_global_shortcuts` / `replace_global_shortcuts` 注册
- 托盘菜单 accelerator **只用于系统菜单右侧展示**（及系统菜单打开时的本地导航习惯，若 OS 支持），不在 tray 模块再次 `GlobalShortcut::register`
- 菜单点击与全局快捷键各自独立触发同一业务函数，允许用户「点菜单」或「按快捷键」两种入口

### 6.3 热更新

在 `save_app_config` 成功且 shortcuts 已替换后，调用托盘刷新函数（例如 `refresh_tray_menu_accelerators`）：

- 按新 config 对 `selection` / `screenshot` / `ocr` / `settings` 的 `MenuItem` 调用 Tauri 提供的 accelerator 更新 API（若版本仅支持重建 Menu，则 `tray.set_menu` 重建并更新 `TrayI18nHandles` 句柄——实现阶段以当前 Tauri 2 API 能力选型，优先原位更新）
- 界面语言变更路径继续只更新文案（`set_text`），加速键以当前 config 为准可一并 set，避免旧 accel 残留

## 7. 架构与模块

### 7.1 主要改动文件

| 文件 | 变更 |
|---|---|
| `src-tauri/src/app/tray.rs` | 重建菜单结构、句柄字段、事件分发、accelerator 应用/刷新 |
| `src-tauri/src/app/shortcuts.rs` | 视需要导出「执行 OcrTranslate / 打开设置」等供 tray 复用的函数，避免 tray 复制粘贴异步编排 |
| `src-tauri/src/ui/i18n.rs` | `apply_interface_language_locked` 更新全部新菜单句柄文案 |
| `src-tauri/src/ui/config.rs` | `save_app_config` 后触发 accelerator 刷新 |
| `frontend/src/i18n/locales/*.json` | 8 语 `tray.*` key |
| 后端 i18n 内置消息（若 tray 文案由 Rust 字典提供） | 与前端 key 对齐；以当前 i18n 架构「谁持有 tray 文案」为准只改一处事实源 |

### 7.2 `TrayI18nHandles` 扩展

```text
TrayI18nHandles {
  tray: TrayIcon,
  selection: MenuItem,
  screenshot: MenuItem,
  ocr: MenuItem,
  settings: MenuItem,
  quit: MenuItem,
  settings_title: Arc<RwLock<String>>,
}
```

旧字段 `translate` 重命名为 `selection`（或保留别名一个版本——本仓库无对外 crate API，直接改名即可）。

### 7.3 动作复用

- **划词翻译 / 打开弹窗**：继续调用 `show_translation_popup`（与原 `translate` 一致）
- **截图翻译**：调用与 `ShortcutAction::OcrTranslate` 相同的入口（`shortcuts.rs` 中现有分支），不得在 tray 内重写 DXGI/overlay 流程
- **文字识别 / 设置 / 退出**：保持现有行为

### 7.4 纯函数（可测）

```text
fn menu_accelerator(shortcuts: &HashMap<String, String>, id: &str) -> Option<String>
// trim 空 → None；非空 → Some(trimmed)
```

可选：`fn tray_menu_bindings() -> &'static [(menu_id, shortcut_id, i18n_key)]` 集中表驱动，减少 setup / refresh 双份列表漂移。

## 8. 错误处理

- 托盘 setup 失败：与现网一致，向上返回 `tauri::Result`，阻止不完整启动或按 `lib.rs` 现有策略处理
- 单次菜单动作失败：日志 + 现有 `show_translation_error` / `log::warn` 模式，不 crash
- 加速键刷新失败：`log::warn`，保留旧菜单，不影响翻译主流程

## 9. 测试与验收

### 9.1 自动化

- `menu_accelerator`：空串 / 空白 / 合法 `Alt+D` / 缺 key
- 若提取表驱动构建逻辑，可断言 menu id 顺序与 shortcut id 映射

### 9.2 手工验收（Windows）

1. 右键托盘：五项 + 两处分隔，顺序与 §5 一致  
2. 默认配置下加速键显示为 `Alt+D` / `Alt+S` / `Alt+O` / `Ctrl+,`；退出无加速键  
3. 设置页清空某一快捷键并保存 → 对应菜单项加速键消失；改回后恢复  
4. 点击：划词打开翻译弹窗；截图进入截图翻译链路；文字识别打开 OCR 窗；偏好设置打开设置；退出退出应用  
5. 切换界面语言 → 菜单文案即时更新且不 reload  
6. 全局快捷键仍可用，与菜单点击不冲突、不双重注册报错  

## 10. 风险与决策记录

| 风险 | 处理 |
|---|---|
| Tauri 2 对已创建 `MenuItem` 是否支持改 accelerator | 实现时先查 API；不支持则重建 menu 并替换句柄 |
| 系统菜单 accelerator 是否在未打开菜单时抢键 | 不额外 global register；若实机发现抢键，改为「文案右侧拼接加速键纯展示、accelerator 参数恒 None」降级（实现计划写清开关条件） |
| `tray.settings` 文案从「设置」改为「偏好设置」 | 产品对齐原型；设置窗口标题 `window.settingsTitle` 本轮不强制改 |

## 11. 成功标准

- 托盘菜单为系统原生外观，信息架构符合方案 3  
- 加速键与 `config.shortcuts` 一致且可热更新  
- 截图翻译可从托盘触发且与 `Alt+S` 同路径  
- i18n 覆盖全部菜单项；OCR 不再硬编码中文  
- 无 WebView 托盘菜单代码进入本轮 diff  
