# WinUI3 翻译弹窗视觉复刻（对齐 Open Design 原型）

> 日期：2026-07-25  
> 状态：**设计已确认，待实现**  
> 规模：**M**（现有 C# 弹窗 UI 视觉复刻；Bridge / Rust 核心不动）  
> 分支：`feature/winui-native-translation-popup`

## 1. 背景与目标

### 1.1 背景

`native/windows/popup` 已具备与 Vue 弹窗功能对等的主路径（源文、语言、多服务结果卡、取消/重试、钉/截图/设置、hide 关窗、Bridge 事件）。界面仍偏「可运行骨架」：宽 420、系统 ThemeResource 实色卡、标题栏按钮集与信息架构未对齐高保真原型。

高保真 SSOT：

| 类型 | 路径 |
|------|------|
| 预览 | `http://localhost:5174/#popup-winui3`（Open Design 工程） |
| 源码 | `OpenDesignProjects/shizi/src/popup/winui3/`（`WinUI3Popup.vue`、`WinUI3Titlebar.vue`、`WinUI3LanguageBar.vue`、`winui3.css`） |
| 共享卡 | 同仓库 `src/popup/components/{SourceCard,ResultCardView,StatusBar}.vue` + `components.css` |

说明：历史文档中「路径 R / Reactor」打磨与当前 **C# 子进程 WinUI** 不是同一实现线。本规格**仅针对** `native/windows/popup`。

### 1.2 目标

1. 将 WinUI 弹窗浅色默认态**视觉与信息架构**对齐 `#popup-winui3`（范围 A：像素级布局/token/层次；控件用 WinUI 等价物）。
2. 保持已有 Bridge 动作可用，不引入新业务子系统。
3. Token 集中可对照原型 CSS，便于后续深色 / Flyout 迭代。

### 1.3 非目标

- 深色主题与标题栏主题切换
- Acrylic 语言搜索 Flyout、入场动画 / items reveal、原型动画调试面板
- 收藏 / 书签（无产品数据模型）— **不展示**
- 引擎 SVG 图标管线（可用色块/首字占位）
- 展开全文 mask 动画（MVP 可用卡内滚动或窗高增高）
- WebView 弹窗外观、设置页 / OCR 迁 WinUI
- Bridge 协议大改、配置热切换 popup 后端

### 1.4 范围档位（已确认）

**A**：视觉像素对齐；语言栏用样式化系统 `ComboBox`；不做 Web 交互逐像素复刻。

## 2. 已确认决策

| 项 | 决定 |
|----|------|
| 施工路径 | Token 资源字典 + 五区布局重写（非单文件糊满、非纯 ThemeResource） |
| 改动栈 | 仅 `native/windows/popup` |
| 宽度 | 逻辑 **468**（现 420 → 468） |
| 主题 | 浅色；Mica 优先，失败 solid 近似 |
| 标题栏动作 | 钉 / 截图 / 设置 / 最小化 / 关闭；**隐藏**收藏、书签、主题 |
| 语言栏 | 一体分段条 + 系统 Combo 列表（无搜索 Flyout） |
| 源文 | 可编辑；去掉主视觉大「翻译」按钮；保留快捷键触发（若已有） |
| 协议 | 默认零 Bridge 字段变更 |

## 3. 架构与模块

### 3.1 分层（不变）

```
Rust core / popup_bridge  ←TCP JSON→  C# BridgeService
                                      ↓
                                 MainWindow UI（本轮只改展示层）
```

- 业务：翻译批次、配置、OCR、历史仍在 Rust。
- 关窗 / 最小化 = **hide**；托盘退出才结束进程。
- 尺寸上报、`SetTitleBar` 拖拽区与按钮命中分离等现网约束保留。

### 3.2 文件落点

```text
native/windows/popup/
  Resources/PopupTokens.xaml   # 新建：色/字号/间距/圆角（对齐 winui3.css 浅色）
  App.xaml                     # Merge PopupTokens
  MainWindow.xaml              # 五区骨架重写
  MainWindow.xaml.cs           # 接线；BuildResultCard 对齐 ResultCardView 结构
```

可选：若 `BuildResultCard` 继续膨胀，再抽 `ResultCardFactory.cs`（不强制本轮）。

### 3.3 硬边界

- 不改翻译协议、配置持久化、历史写入、WebView 弹窗。
- 不把核心逻辑写进 XAML code-behind 新路径；只重组展示与本地视觉状态（如钉 `is-active`）。
- 动作 best-effort：失败 log / 可选 Tip，不抛穿 UI 线程。

## 4. 视觉与布局

### 4.1 Token（浅色 → `PopupTokens.xaml`）

| Token | 值 | 用途 |
|-------|-----|------|
| `PopupFg` | `#1A1A1A` | 主文字 |
| `PopupFg2` | `#5D5D5D` | 次级 |
| `PopupFg3` | `#8A8A8A` | 三级 / model / tokens |
| `PopupAccent` | `#D55A1F` | 柿子橙 |
| `PopupAccentHover` | `#C25016` | accent hover |
| `PopupAccentSoft` | accent @ 10% | badge / 钉激活底 |
| `PopupSuccess` | `#107C10` | 成功 |
| `PopupDestructive` | `#C42B1C` | 关闭 hover / 错误 |
| `PopupBorder` | 黑 @ 6% | 卡描边 |
| `PopupBorder2` | 黑 @ 12% | 重分隔 |
| `PopupHover` | 黑 @ 3.73% | 按钮 hover |
| `PopupCardBg` | 白 @ 72% | 卡 / 语言条表面 |
| `PopupSoft` | 白 @ 55% | 标题栏 soft（可选） |
| `PopupRadiusSm` / `Md` | 4 / 8 | 按钮 / 卡与窗圆角目标 |
| `PopupWindowWidth` | 468 | 逻辑宽 |
| 窗口底 | Mica；失败 `#F4F4F4` @ ~80% | `--w3-mica` |

字体：Segoe UI Variable / Segoe UI。  
阴影：轻卡阴影可用 `Border` + 近似；不强制 CSS blur 半径像素一致。

### 4.2 标题栏（高 44）

对齐 `WinUI3Titlebar.vue`：

- 左：20×20 圆角 5「文」标（accent 底白字）+ `shizi` 13 SemiBold
- 钉：紧挨品牌；36×36；激活 = soft 底 + accent 色
- 中间：`SetTitleBar` **仅**拖拽区（左右按钮在区外，避免命中失效）
- 右工具：截图、设置
- 窗口控件：1px 分隔 + 最小化(hide)、关闭(hide)；关闭 hover = destructive

### 4.3 主体

- 水平 padding **14**，区块 gap **10**；结果列表 gap **8**
- 内容可纵向滚动；窗高继续 `report_content_size` 与工作区上限策略

### 4.4 源文卡

对齐 `SourceCard`：

- 表面：`PopupCardBg` + 1px border + radius 8 + padding `10 12 8`
- 正文在上：透明输入、无边框、约 14px、可编辑
- 底栏 meta：分隔线；左朗读/复制 24×24；右语言 badge（accent + soft pill）
- focus-within：accent 描边
- **去掉**主视觉 Accent「翻译」大按钮；翻译走快捷键 / 既有触发路径

### 4.5 语言栏

对齐 `.w3-lang-bar`（一体分段，非双独立 pill）：

- 满宽容器：card 表面 + border + radius 8
- 源 / 目标：各 `*`，`ComboBox` 去重边框、透明底、13 Medium
- 中部交换 ~38 宽；hover accent
- 下拉：系统 Combo 列表（范围 A）
- 行为：既有 session 语言 + 重译

### 4.6 结果卡

对齐 `ResultCardView`：

| 区 | 规格 |
|----|------|
| 壳 | 同卡表面 |
| 头 | 14×14 引擎占位 + 名（次级 11–12px）+ 状态点 6px + 折叠 chevron |
| 身 | 14px / 行高 ~1.6；可选择；折叠隐藏 body |
| 底 | 朗读 / 复制 /（失败）重试；右 model + `↑in ↓out` |
| meta 规则 | `microsoft_edge` 不展示 model/tokens（同现网） |
| 溢出 | MVP：不强制 mask+「展开全文」；长文卡内滚动或窗增高 |

状态：Finished 不写「成功」字；Failed 用 destructive 标题 + message；Translating 无字时头点 + 弱占位。

### 4.7 状态栏

- 高约 28–32（含 padding）；透明底；顶 1px border
- 左：6px 状态点 + 文案；条件「取消」/「重试」（accent 链）
- 右：`N 字`
- 主指示不用大 `ProgressRing`（可改为点）；Tip/InfoBar 可保留弱提示

### 4.8 材质与动效

- Mica 优先；失败 solid 近似
- 不做深色、入场动画、items reveal、调试面板、用户可拖拽改窗尺寸（仍 `IsResizable=false`）

### 4.9 验收口径（像素级 A）

1. 与 `#popup-winui3` **浅色默认态**并排：宽、标题栏结构、源文层次、一体语言条、结果卡头身底、状态栏点+字数，视觉同级。
2. 不要求：字体 hinting、阴影 blur、Combo 内部、引擎 SVG 与原型逐像素相同。
3. 既有 Bridge 动作不回归。

## 5. 动作与状态

### 5.1 动作映射

| UI | 行为 |
|----|------|
| 钉 | 既有置顶 toggle + 视觉 active |
| 截图 | 既有 OCR/截图译（先 hide） |
| 设置 | 既有 open_settings |
| 最小/关闭 | hide |
| 源文编辑 | 既有；Ctrl+Enter 等既有触发保留 |
| 朗读/复制 | 既有 |
| 语言/交换 | 既有 |
| 折叠/复制/朗读/卡侧重试 | 既有（按钮布局对齐原型底栏） |
| 状态取消 | 仅翻译中显示 |
| 状态重试 | 非翻译中且存在失败/取消卡时显示 |
| 收藏/书签/主题 | 不展示 |

协议默认零变更；钉状态用窗口本地字段即可。

### 5.2 UI 状态摘要

| 场景 | 表现 |
|------|------|
| 空闲无源文 | 「就绪」；结果可空 |
| 翻译中 | accent 点 +「翻译中…」+ 取消 |
| 完成 | 完成文案；点可用 success 或静态 accent |
| 失败 | 卡 destructive；状态栏可重试 |
| 源语言 badge | 检测/会话源语言标签 |

## 6. 测试与验证

| 层 | 内容 |
|----|------|
| 单测 | 既有 State/语言单测不破坏；可选补纯函数（meta 是否展示、状态栏 action 种类） |
| 编译 | `dotnet build` popup 工程（x64） |
| 手工 | `popupUi=winui`：划词、钉/截图/设置/关、多卡、取消失败、换语言；并排对照原型 |

不做：截图像素 CI、深色矩阵、动画矩阵。

## 7. 实现顺序

1. `PopupTokens.xaml` + App 合并 + 窗宽 468 + Mica/回退底  
2. 标题栏布局与按钮集  
3. 源文卡结构  
4. 语言栏一体分段皮  
5. `BuildResultCard` 重组  
6. 状态栏点 + 条件动作  
7. 编译 + 手工对照  

## 8. 成功标准

1. 浅色 WinUI 弹窗与 Open Design `#popup-winui3` 并排视觉同级（范围 A）。  
2. 钉、截图、设置、hide、语言、翻译流、复制/朗读、取消/重试可用且无 Bridge 回归。  
3. 无收藏/书签/主题死按钮；无深色与动画调试范围蔓延。  
4. 文档：本规格；实现完成后视需要同步 `docs/agent/architecture-notes.md` 中 WinUI UI 描述（若有过时「骨架」表述）。

## 9. 与历史文档关系

- `2026-07-24-winui-native-translation-popup-design.md`：子系统与 Bridge（已实现）。本规格是其上的**视觉层**增量。  
- 历史 `winui-reactor-popup-polish`：路径 R，**不适用**当前 C# 工程。
