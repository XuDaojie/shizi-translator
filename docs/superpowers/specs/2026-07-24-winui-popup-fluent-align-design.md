# WinUI 原生翻译弹窗对齐 Open Design WinUI3 原型

## 背景

产品侧 Windows 翻译弹窗双后端中，`popupUiBackend=winui` 走**路径 B：Win32 + GDI 自绘**（非 XAML / 不强制 WinAppSDK）。当前原生表面仍是「源文 + 列表 + 底栏文字按钮」暖色板布局，与 Open Design 高仿真原型 `popup-winui3`（Fluent 2 / WinUI 3 取向）差距大。

原型仓库：`OpenDesignProjects/shizi`，入口 `#popup-winui3`。视觉 SSOT：`src/popup/winui3/*` + 共用 `components.css` / `popup-tokens.css`。

## 目标

1. **北极星**：尽量像素级复刻原型 WinUI3 弹窗（布局、token、组件结构、交互）。
2. **本次（A 阶段）**：结构 + 视觉对齐，且**按 C 的骨架施工**，避免过渡 UI 推倒重来。
3. **A 必须包含**：语言栏展示、**交换**、**简单语言列表选择**（触发已有 `SetSessionLanguages`）。
4. 业务仍在 Rust 核心；UI 只展示 + 转发 `PopupUserAction`。

## 非目标（A）

- 真实 Mica / Acrylic 模糊材质（可用实色近似）。
- 结果卡折叠 / 展开全文 / 溢出 mask 动画。
- 入场动画、items reveal。
- 深色主题完整版。
- 引入 WinAppSDK / XAML / 改 WebView 弹窗外观。
- 原型「动画调试面板」。

## 阶段路线

| 阶段 | 内容 | 验收 |
|------|------|------|
| **A（本文）** | 五区布局 + Fluent 浅色 token + 图标标题栏 + 源文卡 + 语言栏（含列表）+ 结果卡 + 状态栏；主路径动作 | 并排原型：同骨架、同色调 |
| **B1** | 语言 flyout 搜索、键盘导航 | 与原型语言栏一致 |
| **B2** | 折叠/展开、溢出、卡片 hover | 结果卡交互 |
| **B3** | 真 Mica/高光、入场/reveal | 材质与动效 |
| **C** | 深色、缺失动作接线、间距扫尾 | 像素级清单 |

## A 阶段布局（与原型同构）

```
标题栏 44px：品牌 + 钉/收藏/截图/书签/设置/主题 + 最小化/关闭
body pad 14 / gap 10：
  源文卡 → 语言栏（源 ▾ | ⇄ | 目标 ▾）→ 结果卡列表（可滚动）
状态栏 ~28px：状态文案（可含取消/重试热区）+ 字数
```

- 逻辑宽 **468**（由 420 调整）；高仍以上限 + 卡片区滚动为主（默认逻辑高可保持 480 或随内容策略，定位常量同步）。
- 圆角 8：继续 DWM `DWMWCP_ROUND` + 卡片矩形近似。

## 视觉 token（浅色，COLORREF 实现）

| 用途 | 值 |
|------|-----|
| 窗底 | `#F4F4F4` |
| 卡片 | `#FFFFFF` + 边 `≈ rgba(0,0,0,0.06)` |
| 前景 | `#1A1A1A` / `#5D5D5D` / `#8A8A8A` |
| accent | `#D55A1F` |
| 成功/警告/危险 | `#107C10` / `#CA5010` / `#C42B1C` |
| 字体 | Segoe UI，13 / 12 / 11 级 |

## 交互（A）

**已接线**

| 控件 | 动作 |
|------|------|
| 关闭 / Esc | `Close` |
| 设置 | `OpenSettings` |
| 取消（翻译中，状态栏热区） | `CancelTranslation` |
| 重试（有失败且非翻译中） | `Retry` |
| Ctrl+C / 复制语义 | `CopyResult`（首张可复制卡） |
| 语言交换 | `SetSessionLanguages`（auto 规则对齐原型） |
| 语言列表点选 | `SetSessionLanguages` + 关闭列表 |

**结构占位（no-op 或仅重绘，C 接线）**

钉住、收藏、截图、书签、主题、最小化。

**语言列表**

- 内嵌在弹窗客户区绘制（非独立 HWND），点源/目标打开，再点关闭或点列表外关闭。
- 源语言含 `auto`；目标语言不含 `auto`。
- 显示名与前端 `translation-languages` 对齐（硬编码表即可）。
- A 可不做搜索框；列表可滚动。

## ViewModel / 绘制快照

- 复用 `PopupViewModel` / `PopupUserAction`。
- `PaintCardSnapshot` 补充 `usage_input` / `usage_output`（可选），用于结果卡 model + tokens 行。
- 折叠态不进 A 的 VM 必改范围。

## 实现落点

- 主改：`src-tauri/src/app/popup_backend/winui/ui.rs`（布局 / 绘制 / 命中 / 语言列表状态）。
- 动作：`actions.rs` 已支持 `SetSessionLanguages` 等，尽量不扩协议。
- 测试：布局命中、语言 label、token 色、快照字段；`cargo test`（default features）。

## 风险与约束

- GDI 无法 1:1 还原 backdrop-filter；A 用实色 Mica 近似。
- 图标以几何线段绘制，不必 SVG 像素一致。
- 热键线程仍禁止在 UI 回调阻塞；换语言重译保持独立线程。

## 成功标准（A）

1. `cargo test` 通过（含 winui 相关单测）。
2. `popupUiBackend=winui` 下目视：宽约 468、五区顺序、橙 accent、结果卡有引擎名与正文、底为状态栏而非旧底栏按钮条。
3. 可打开语言列表并选择，交换语言生效。
4. 关闭 / 设置 / 取消 / 重试 / 复制主路径可用。
