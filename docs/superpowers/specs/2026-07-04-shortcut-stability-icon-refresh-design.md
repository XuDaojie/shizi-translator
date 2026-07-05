# 快捷键稳定性与图标更新设计

## 背景

当前全局快捷键存在两类问题：

1. 触发稳定性不足。用户反馈 `Alt+T` 在 Chrome 中有时会先把焦点切到地址栏或触发窗口切换，随后快捷键行为又恢复，表现像“偶发失效”。
2. 配置链路不完整。前端保存设置时，`projectToAppConfig()` 目前把 `shortcuts` 固定写成空对象；前端启动同步后端配置时，也没有把后端 `shortcuts` 合并回设置页状态。这会让快捷键配置在“设置页 localStorage / 后端 config.json / 实际运行时注册”三处出现漂移。

同时，产品希望把默认快捷键改成与 Bob 一致的：

- 划词翻译：`Alt+D`
- 截图翻译：`Alt+E`

视觉上，Windows 窗口左上角与系统托盘仍在使用旧应用图标。用户已确认本轮采用“字形导向”的方案 A：深色底块上的 `文A` 双字徽记。

## 目标范围

### 必须实现

- 全局快捷键触发逻辑改为更稳定的事件分支，优先修复“已注册但偶发不触发”的问题。
- 前后端快捷键配置打通：设置页保存、启动同步、运行时注册三处使用同一份 `shortcuts` 数据。
- 新默认快捷键改为 `Alt+D` / `Alt+E`，同时保留设置页可修改。
- 仅对“仍在使用旧默认值且未自定义”的用户自动迁移到新默认值；已自定义的用户不覆盖。
- Windows 窗口左上角应用图标与系统托盘图标更新为同一语义的翻译图标。

### 明确不做

- 不新增快捷键冲突推荐器、平台差异提示、快捷键预设方案管理。
- 不为安装包额外设计第二套品牌稿；沿用同一图标资源。
- 不改 `word-lookup` 的实现状态，它仍然只保留配置保存能力。
- 不参考或翻译 `pot-desktop/` 代码实现。

## 方案对比与定稿

### 快捷键方案

1. 只修事件触发，不改默认值。
2. 修事件触发，改默认值，并对旧默认未自定义用户自动迁移。
3. 强制把所有用户重置到新默认。

本轮选择方案 2。

原因：

- 能解决当前“偶发不触发”。
- 能把产品默认值切到 Bob 习惯。
- 不覆盖已经手改过快捷键的用户。

### 图标方案

用户已确认采用视觉候选 A：

- 深色圆角底块
- 中心 `文A` 双字徽记
- 小尺寸优先，托盘 16px 可辨识度优先于大尺寸细节

不选 B / C 的原因：

- B 上下排布在托盘尺寸下更容易糊。
- C 作为品牌徽章更完整，但小尺寸细节损失最大。

## 根因判断

本轮按“最小但正确”的思路处理两个真正会造成问题的点：

### 1. 事件分支过于保守

`src-tauri/src/app/shortcuts.rs` 当前只在 `ShortcutState::Released` 时执行动作。`tauri-plugin-global-shortcut 2.3.2` 官方示例使用的是 `Pressed`。对 `Alt+...` 这类组合键，当前窗口或浏览器如果在按下阶段先改变了焦点或菜单状态，`Released` 事件更容易表现不稳定。

设计结论：

- 运行时只处理 `Pressed`。
- 不做复杂防抖层，先用插件建议路径解决根因。

### 2. 快捷键配置链路断裂

- 前端投影到后端时丢弃 `shortcuts`。
- 前端 `syncFromBackend()` 只合并 `services`，忽略后端 `shortcuts`。
- 结果是“设置页看起来改了”与“后端实际注册了什么”可能不一致。

设计结论：

- `config.json` 继续作为后端快捷键事实来源。
- 前端保存时显式投影 `state.shortcut.bindings` 到 `AppConfig.shortcuts`。
- 前端启动同步时显式把 `backend.shortcuts` 合并回设置页状态。

## 数据模型

后端 `AppConfig.shortcuts` 继续使用 `id -> keys` 的 map，不新增新结构。

本轮默认值改为：

| id | 新默认快捷键 | 行为 |
| --- | --- | --- |
| `translate-selection` | `Alt+D` | 划词翻译 |
| `translate-clipboard` | `Ctrl+Shift+C` | 剪贴板翻译 |
| `translate-screenshot` | `Alt+E` | 截图翻译 |
| `word-lookup` | 空 | 仅保存，不注册 |
| `show-window` | `Ctrl+Shift+Space` | 显示主窗口 |
| `open-settings` | `Ctrl+,` | 打开设置 |

兼容规则：

- 缺失字段补新默认值。
- 空字符串仍表示禁用。
- 已显式保存的自定义值原样保留。

## 自动迁移规则

迁移只覆盖两个动作：

- `translate-selection`：`Alt+T` -> `Alt+D`
- `translate-screenshot`：`Alt+O` -> `Alt+E`

迁移条件必须同时满足：

1. 当前值等于旧默认值。
2. 当前值不等于新默认值。
3. 对应目标动作没有被用户改成其他值。

换句话说：

- 旧默认用户自动升级。
- 已自定义成别的组合键的用户不动。
- 已经改成新默认的用户不重复迁移。

迁移落点：

- 后端 `AppConfig.normalized()` 对 `shortcuts` 执行一次迁移，保证运行时读取的配置就是最终值。
- 前端从后端同步时接收迁移后的值，设置页展示与实际注册保持一致。

本轮不在 localStorage 里单独做第二套迁移器，避免前后端各自维护一份迁移逻辑。

## 后端设计

### 1. `src-tauri/src/app/shortcuts.rs`

- `handle_global_shortcut()` 改为只响应 `ShortcutState::Pressed`。
- 其他动作分发逻辑不改，继续复用现有划词、截图 OCR、剪贴板翻译和主窗口显示链路。

### 2. `src-tauri/src/core/config/types.rs`

- `default_shortcuts()` 中把 `translate-selection` / `translate-screenshot` 的默认值改为 `Alt+D` / `Alt+E`。
- `normalize_shortcuts()` 中加入“旧默认 -> 新默认”的条件迁移。
- 单测覆盖新默认值和迁移规则。

后端保持单一职责：

- 归一化与迁移由配置层处理。
- 注册与触发由快捷键层处理。

## 前端设计

### 1. `frontend/src/lib/config.ts`

`projectToAppConfig()` 不再返回 `shortcuts: {}`，而是把 `state.shortcut.bindings` 投影为：

- key：`binding.id`
- value：`binding.keys.trim()`

这样设置页保存任意配置时，不会把后端快捷键配置冲掉。

### 2. `frontend/src/settings/stores/settings.ts`

`syncFromBackend()` 除了合并 `services`，还要把 `backend.shortcuts` 合并回 `state.shortcut.bindings`：

- `label` / `description` 继续以前端默认定义为准。
- `keys` 以后端值为准。
- `error` 在同步后清空，避免保留旧的前端校验状态。
- 后端缺失的已知 id 使用前端默认定义兜底。

### 3. 前端默认值

`buildDefaults()` 中的两条默认绑定同步改为：

- `translate-selection` -> `Alt+D`
- `translate-screenshot` -> `Alt+E`

这样在 Tauri 未就绪或首次启动时，设置页展示也与产品默认保持一致。

## 图标设计

### 1. 窗口与托盘应用图标

托盘当前使用 `app.default_window_icon()`。因此只要替换 `src-tauri/icons/icon.ico`，托盘图标就会一起更新。

这是本轮选择的最短路径：

- 不额外引入托盘专用图标分支
- 不新增动态绘制逻辑
- 接受“窗口/托盘/应用图标资源保持一致”的副作用
- 不修改设置页导航栏或侧边栏品牌元素

### 2. 资源策略

维护一套统一的字形稿：

- Tauri `bundle.icon` 使用 `icons/icon.ico`
- Tauri 托盘通过 `app.default_window_icon()` 复用同一应用图标

本轮不新增图标构建流水线，也不引入新的设计依赖。实现阶段可以手工生成最终 `icon.ico`，但视觉语义只维护一套。

## 数据流

### 启动

1. 后端 `ConfigStore::load()` 读取 `config.json`。
2. `AppConfig.normalized()` 补默认值并执行旧默认迁移。
3. `run()` 调 `register_global_shortcuts()`，按迁移后的最终配置注册。
4. 设置页挂载后调用 `syncFromBackend()`，把后端快捷键值合并回前端状态。

### 保存

1. 用户在设置页修改快捷键。
2. 前端 `projectToAppConfig()` 把 `bindings` 投影到 `shortcuts`。
3. 后端 `save_app_config()` 调 `replace_global_shortcuts()` 先试注册，再写配置。
4. 成功后前后端显示同一份配置。

## 验收标准

- 启动后，未自定义旧默认值的用户会看到 `Alt+D` / `Alt+E`，且两者可立即触发对应动作。
- 已自定义快捷键的用户，升级后不会被重置到新默认值。
- 在设置页修改任意快捷键并保存后，无需重启即可生效。
- 保存其他配置项时，不会把已有快捷键配置清空。
- Windows 窗口左上角出现新的 `文A` 应用图标。
- 系统托盘图标更新为同语义的翻译图标。

## 测试与验证

### Rust 单测

- `default_shortcuts()` 输出 `Alt+D` / `Alt+E`。
- 旧默认值 `Alt+T` / `Alt+O` 在满足条件时自动迁移。
- 已自定义值不会被迁移覆盖。
- 触发分类测试继续通过。

### 前端单测

- `projectToAppConfig()` 正确输出 `shortcuts`。
- `syncFromBackend()` 会把后端 `shortcuts` 合并回设置页状态。
- 保留前端文案字段，只覆盖 `keys`。

### 构建验证

```bash
npm run test
npm run typecheck
cd src-tauri && cargo test
cd src-tauri && cargo build
```

### 手动验证

1. 在 Chrome 中聚焦网页，按 `Alt+D` 触发划词翻译，观察是否仍出现“偶发无响应”。
2. 修改快捷键为自定义值，保存后立即验证新键生效、旧键失效。
3. 重启应用，确认自定义值与迁移后的默认值都能持久化。
4. 查看 Windows 窗口左上角与系统托盘图标，确认视觉语义一致。

## 风险与取舍

- `Alt+D` 本身是 Windows 浏览器常见快捷键，比 `Alt+T` 更容易与地址栏语义重叠。本轮因为产品明确要求与 Bob 一致，所以接受这个默认值；设置页保留可改能力作为兜底。
- 仅切到 `Pressed` 已经覆盖当前最可能根因，但不承诺解决所有第三方软件对 `Alt` 组合键的抢占问题。
- 托盘图标复用应用图标资源是最短路径，但这意味着后续若要“托盘专用单色版”，需要再拆分资源。

## 文档同步

编码完成后需要同步：

- `README` 中的默认快捷键说明。
- `AGENTS.md` / `CLAUDE.md` 中与快捷键默认值相关的描述。
- 与应用图标资源、窗口图标、托盘图标相关的说明文档（如果已有对应条目）。

## 规格自检

- 无 TODO / 待定占位。
- 明确了为什么修 `Pressed`、为什么补 `shortcuts` 链路、为什么只迁移旧默认未自定义用户。
- 图标范围限定在“Windows 窗口左上角 + 系统托盘”，没有扩散到设置页导航栏或无关页面。
- 范围可由一个实现计划覆盖。
