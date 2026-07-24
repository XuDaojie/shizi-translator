# 路径 R WinUI 3 翻译弹窗界面打磨设计

## 背景

路径 R（`windows-reactor` 真 WinUI 3）已完成宿主生命周期、五区骨架与主路径动作接线（源文 / 语言 Combo / 多服务结果卡 / 取消重试 / 设置关闭、Mica 与 accent 初版）。当前 UI 仍偏「可运行骨架」：标题栏为文字按钮、源文与结果缺乏卡片层次、与 Open Design `#popup-winui3` 视觉差距大，钉住 / 截图等译入口为 stub。

本设计在**不推翻路径 R 契约**的前提下，将弹窗打磨为浅色 Fluent 产品级体验。

## 目标

1. 视觉与信息架构逼近 Open Design `#popup-winui3`（浅色）：图标标题栏、源文卡、语言栏、结果卡层次、状态栏。
2. 标题栏**可产品化动作**真接线：钉住（置顶）、截图译、设置、最小化/关闭。
3. 语言栏在**系统 ComboBox** 上抛光（非自建 Flyout + 搜索）。
4. Reactor UI 分区模块化 + 集中 Fluent token，便于后续深色/搜索迭代。
5. 业务仍在 Rust core；UI 只展示与转发 `PopupUserAction`。

## 非目标（本轮）

- 深色主题完整版与主题切换接线。
- 语言 Flyout + 搜索、复杂入场动画 / items reveal、原型动画调试面板。
- 收藏、书签（产品无数据模型）——**不展示**。
- 源文就地编辑触发重译（保持只读 + selectable）。
- 结果卡折叠/展开全文动画、与 WebView Bob 风弹窗像素一致。
- 设置 / OCR overlay 迁 Reactor；backend 热切换；GDI 回退（已移除）。
- NSIS self-contained Runtime 变更。

## 规模

**M 档**：~ 多文件 UI 拆分 + 薄协议扩展 + actions 接线；边界清晰，本对话可实现。不默认升 L、不强制独立 plan（步骤多时可再拆任务清单）。

## 已确认决策

| 项 | 决定 |
|----|------|
| 范围姿态 | 全面逼近原型，但砍调试与无模型能力 |
| 主题/动效 | 浅色核心 + 可选极轻 show 透明度；深色与复杂动画延后 |
| 标题栏 | 钉 / 截图 / 设置 / 最小化 / 关闭；**隐藏**收藏、书签、主题 |
| 语言栏 | 增强系统 ComboBox |
| 施工 | 分区组件 + `tokens.rs`（非单文件糊满） |

## 架构

### 分层（不变）

```
TranslationEvent → PopupHost → WinuiPopupBackend.publish
  → ReactorHostHandle → hooks 状态 → render_popup(vm)

用户点击 → PopupUserAction → actions::handle_user_action
  → core / 既有命令（best-effort）
```

- `PopupBackend` / `PopupHost` 契约不变。
- 关窗 = hide；托盘退出才结束进程。
- Runtime 失败仍 `create_host_with_winui_fallback` → WebView。

### 模块落点

```
src-tauri/src/app/popup_backend/
  types.rs                 # + TogglePin, TriggerOcr
  winui/
    actions.rs             # 置顶、触发 OCR、既有动作
    reactor/
      tokens.rs            # 新建：色/字号/间距/圆角
      view.rs              # 总装 render_popup
      title_bar.rs         # 新建：品牌 + 图标按钮
      source_card.rs       # 新建：源文卡
      language_bar.rs      # 抽出 + Combo 抛光
      result_cards.rs      # 抽出 + 卡片层次
      status_bar.rs        # 抽出
      host.rs / state.rs   # 生命周期基本不动
      langs.rs / meta.rs   # 复用
```

### 硬边界

- 不把翻译协议、配置持久化、历史写入放进 UI 层。
- 不引入 .NET / 独立 XAML 文件。
- 禁止在全局快捷键回调同步栈做重初始化；`publish` 非阻塞。

## 视觉与布局

### SSOT

- Open Design：`OpenDesignProjects/shizi` → `#popup-winui3`、`src/popup/winui3/*`、`winui3.css` 浅色 token。
- 逻辑宽 **468**；标题栏 ~44px；body pad ~14 / gap ~10；状态栏 ~28px。
- 结果区继续 `scroll_viewer` + 合理 max-height；窗高策略保持 host 现有 `inner_size` 或微调。

### Token（浅色，`tokens.rs`）

| 用途 | 值 |
|------|-----|
| accent | `#D55A1F`（柿子橙）+ on-accent 白 |
| fg | `#1A1A1A` / `#5D5D5D` / `#8A8A8A` |
| card | 白/高不透明白 + border ≈ `rgba(0,0,0,0.06)` |
| success / warning / danger | `#107C10` / `#CA5010` / `#C42B1C` |
| radius | 卡 8；语言触发外观尽量 pill |
| 字体 | Segoe UI 系；13/12/11 级 |

Reactor 侧用 `Color::rgb` 与控件 builder 属性表达；无法 1:1 的半透/模糊处用最接近的实色/系统表面。

### 五区要点

1. **标题栏**  
   - 品牌：柿子橙「文」标 + `shizi`（或产品中文名择一，默认与原型 `shizi` 一致，可与现窗标题「柿子翻译」并存：窗标题保持可 FindWindow）。  
   - 图标按钮：钉（左）、截图、设置、最小化、关闭（右）；**非**「钉」「收藏」等文字按钮。  
   - 可拖动：在 Reactor/WinUI 能力范围内保留 caption/drag 区域。

2. **源文卡**  
   - 卡片表面 + 弱标签「源文」；正文 selectable 只读。  
   - 空源文：占位弱文案或状态栏「就绪」，避免刺眼「（无源文）」大字（可弱化为 caption）。

3. **语言栏**  
   - 源 Combo + 圆形/图标化 ⇄ + 目标 Combo。  
   - 列表数据与交换规则继续 `langs.rs`（源含 auto，目标不含）。  
   - 本轮不实现搜索框。

4. **结果卡**  
   - 服务名、状态色（等待/翻译中/失败/取消；完成不刷状态字）、正文、model、tokens、复制。  
   - `microsoft_edge`：不展示 model/tokens（`meta.rs` 规则）。  
   - 多卡保序 + 滚动。

5. **状态栏**  
   - 状态点 + 文案（翻译中… / 就绪 / 完成）+ **条件**取消或重试 + 字数。  
   - 取消：仅 `is_translating`。  
   - 重试：非翻译中且存在 Failed/Cancelled 卡时显示（整批 `Retry { service_instance_id: None }`）。

### 材质与动效

- 继续 Mica（host 已有路径上抛光，不回退纯色糊墙除非 API 失败）。  
- 可选：show 时极轻 opacity；**不做** items reveal / 调试面板。

## 动作与状态

### `PopupUserAction`

| 变体 | 行为 |
|------|------|
| `Close` | `host.hide()`；最小化与关闭共用 |
| `OpenSettings` | 既有 `request_show_settings_window` |
| `CancelTranslation` | 既有 cancel |
| `Retry { service_instance_id }` | 既有整批重试（id 预留） |
| `CopyResult { service_instance_id }` | 既有剪贴板 |
| `SetSessionLanguages { … }` | 既有 + 可重试输入时后台重译 |
| **`TogglePin`（新）** | 切换弹窗 HWND 置顶；UI active 样式；失败 log |
| **`TriggerOcr`（新）** | 复用 `shortcuts::trigger_ocr_translate`（或等价）；先 hide 弹窗再走截图译，避免挡 overlay |

钉状态：优先 host/`SharedUi` 侧 `AtomicBool` 或等价，**不强制**写入 `PopupViewModel`（避免每次 translation event 冲掉）；render 时读取。

### 错误

- 全部动作 best-effort：失败只 `log`，不 panic UI 线程。  
- 无 AppHandle 绑定时忽略动作并 warn（现状）。

## 测试

1. **纯函数**：footer 文案、card body、status 色映射、meta tokens、语言交换（既有 + 回归）。  
2. **渲染冒烟**：`render_popup` 多卡/空卡返回 `Element`（既有模式）。  
3. **动作分发**：`TogglePin` / `TriggerOcr` 分支可测（mock 或抽纯调度）。  
4. **可选 GUI**：`SHIZI_M0_SPIKE` 冒烟不进默认 CI 硬依赖。  
5. 默认 `cargo test`（Windows + `popup-winui`）通过。

## 成功标准

1. `popupUiBackend=winui` 且 Runtime 可用时，目视：宽约 468、五区、图标标题栏、源文/结果卡表面、橙 accent、Mica 不回退到 GDI 文字条。  
2. 钉住可切换置顶；截图译能进入既有 OCR 路径；设置/关闭/语言/复制/取消/重试主路径可用。  
3. 无收藏/书签/主题按钮。  
4. `cargo test` 相关模块通过。  
5. 架构/本 spec 与代码模块名一致；不引入业务逻辑进 view。

## 与既有文档关系

| 文档 | 关系 |
|------|------|
| `2026-07-24-winui-reactor-popup-design.md` | 路径 R 总设计；本 spec 为 **M3 后界面打磨** 增量 |
| `2026-07-24-winui-popup-fluent-align-design.md` | GDI 时代视觉对照；token 可复用，实现落点改为 Reactor |
| `docs/agent/architecture-notes.md` | 实现收尾时补一句：路径 R UI 分区与本轮动作扩展 |

## 实现顺序建议

1. `tokens.rs` + 从 `view.rs` 抽出五区文件（行为先等价迁移）。  
2. 标题栏图标化 + 隐藏无能力按钮。  
3. 源文卡 / 结果卡 / 状态栏视觉层次。  
4. 语言栏 Combo 外观抛光。  
5. `TogglePin` / `TriggerOcr` 协议与 `actions` 接线。  
6. 单测 + 文档同步。

## 风险

| 风险 | 缓解 |
|------|------|
| Reactor 控件 API 对圆角/半透支持有限 | token 降级为实色；不强求 CSS 级 backdrop |
| 置顶 API 与 STA 线程 | 仅在 UI 线程或经 host 投递改 HWND |
| OCR 触发与弹窗叠层 | TriggerOcr 先 hide 再调既有入口 |
| view 拆分引入循环依赖 | 保持 view→actions 函数指针模式，子模块不反向依赖 host |
