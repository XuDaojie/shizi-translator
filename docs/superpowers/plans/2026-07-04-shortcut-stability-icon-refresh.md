# 快捷键稳定性与图标更新实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 修复全局快捷键触发与配置同步漂移，把默认快捷键迁移到 `Alt+D` / `Alt+E`，并统一 Windows 窗口左上角与系统托盘图标语义。

**架构：** 后端 `AppConfig.normalized()` 继续作为快捷键默认值与迁移的唯一入口，`shortcuts.rs` 只负责注册和按 `Pressed` 事件分发。前端只把设置状态投影到后端 `shortcuts`，启动时用后端返回值覆盖 `keys`，不再维护第二套迁移器。图标走最短路径：`tauri.conf.json` 声明 `bundle.icon = ["icons/icon.ico"]`，托盘继续通过 `app.default_window_icon()` 复用同一应用图标资源。

**技术栈：** Rust / Tauri 2 / tauri-plugin-global-shortcut 2 / Vue 3 / Vitest / PowerShell .NET 图标生成。

---

## 文件结构

- 修改：`src-tauri/src/core/config/types.rs`
  - 定义快捷键默认值、保留空字符串禁用语义、迁移旧默认 `Alt+T` / `Alt+O`。
  - 增加 Rust 单测覆盖默认值、旧默认迁移、自定义值不覆盖。
- 修改：`src-tauri/src/app/shortcuts.rs`
  - `handle_global_shortcut()` 从 `ShortcutState::Released` 改为只处理 `ShortcutState::Pressed`。
  - 现有 `classify_shortcut()` 单测继续覆盖动作映射。
- 修改：`frontend/src/lib/config.ts`
  - `projectToAppConfig()` 将 `state.shortcut.bindings` 投影为后端 `shortcuts`。
- 修改：`frontend/src/lib/config.test.ts`
  - 增加投影快捷键、保留空字符串的单测。
- 修改：`frontend/src/settings/stores/settings.ts`
  - 默认 `translate-selection` 改 `Alt+D`，`translate-screenshot` 改 `Alt+E`。
  - 新增 `mergeBackendIntoShortcuts()`，`syncFromBackend()` 在后端非空时同步快捷键。
- 修改：`frontend/src/settings/stores/settings.test.ts`
  - 增加默认快捷键和后端快捷键合并测试。
- 修改：`src-tauri/tauri.conf.json`
  - 声明 `bundle.icon = ["icons/icon.ico"]`，确保窗口左上角使用应用图标。
- 修改：`src-tauri/icons/icon.ico`
  - 更新为同语义 `文A` 深色底块图标；托盘通过 `app.default_window_icon()` 复用。
- 修改：`README.md`（如存在）、`AGENTS.md`、`CLAUDE.md`
  - 同步默认快捷键说明为 `Alt+D` / `Alt+E`。

---

### 任务 1：后端快捷键默认值与迁移

**文件：**
- 修改：`src-tauri/src/core/config/types.rs:94-130`
- 测试：`src-tauri/src/core/config/types.rs:158-356`

- [ ] **步骤 1：编写失败的 Rust 单测**

在 `#[cfg(test)] mod tests` 内追加：

```rust
#[test]
fn defaults_shortcuts_use_bob_style_keys() {
    let config = AppConfig::from_env();

    assert_eq!(config.shortcuts.get("translate-selection").map(String::as_str), Some("Alt+D"));
    assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+E"));
    assert_eq!(
        config.shortcuts.get("translate-clipboard").map(String::as_str),
        Some("Ctrl+Shift+C")
    );
    assert_eq!(config.shortcuts.get("word-lookup").map(String::as_str), Some(""));
    assert_eq!(
        config.shortcuts.get("show-window").map(String::as_str),
        Some("Ctrl+Shift+Space")
    );
    assert_eq!(config.shortcuts.get("open-settings").map(String::as_str), Some("Ctrl+,"));
}

#[test]
fn normalized_migrates_old_default_shortcuts() {
    let mut config = AppConfig::from_env();
    config.shortcuts.insert("translate-selection".to_string(), "Alt+T".to_string());
    config.shortcuts.insert("translate-screenshot".to_string(), "Alt+O".to_string());

    let config = config.normalized();

    assert_eq!(config.shortcuts.get("translate-selection").map(String::as_str), Some("Alt+D"));
    assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some("Alt+E"));
}

#[test]
fn normalized_keeps_custom_shortcuts_and_empty_disabled_bindings() {
    let mut config = AppConfig::from_env();
    config.shortcuts.insert("translate-selection".to_string(), "Ctrl+Alt+T".to_string());
    config.shortcuts.insert("translate-screenshot".to_string(), "".to_string());

    let config = config.normalized();

    assert_eq!(
        config.shortcuts.get("translate-selection").map(String::as_str),
        Some("Ctrl+Alt+T")
    );
    assert_eq!(config.shortcuts.get("translate-screenshot").map(String::as_str), Some(""));
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cd src-tauri && cargo test core::config::types::tests
```

预期：至少前两个测试失败，因为当前默认值仍是 `Alt+T` / `Alt+O`，且 `normalized()` 会删除空快捷键。

- [ ] **步骤 3：编写最少后端实现**

在 `AppConfig::from_env()` 创建 `shortcuts` 前增加两个小函数：

```rust
fn default_shortcuts() -> HashMap<String, String> {
    HashMap::from([
        (
            "translate-selection".to_string(),
            env::var("SHIZI_SHORTCUT_TRANSLATE_SELECTION").unwrap_or_else(|_| "Alt+D".to_string()),
        ),
        (
            "translate-screenshot".to_string(),
            env::var("SHIZI_SHORTCUT_TRANSLATE_SCREENSHOT").unwrap_or_else(|_| "Alt+E".to_string()),
        ),
        (
            "translate-clipboard".to_string(),
            env::var("SHIZI_SHORTCUT_TRANSLATE_CLIPBOARD")
                .unwrap_or_else(|_| "Ctrl+Shift+C".to_string()),
        ),
        (
            "word-lookup".to_string(),
            env::var("SHIZI_SHORTCUT_WORD_LOOKUP").unwrap_or_else(|_| "".to_string()),
        ),
        (
            "show-window".to_string(),
            env::var("SHIZI_SHORTCUT_SHOW_WINDOW")
                .unwrap_or_else(|_| "Ctrl+Shift+Space".to_string()),
        ),
        (
            "open-settings".to_string(),
            env::var("SHIZI_SHORTCUT_OPEN_SETTINGS").unwrap_or_else(|_| "Ctrl+,".to_string()),
        ),
    ])
}

fn normalize_shortcuts(mut shortcuts: HashMap<String, String>) -> HashMap<String, String> {
    let defaults = default_shortcuts();
    let mut normalized = HashMap::new();

    for (id, default_keys) in defaults {
        let keys = shortcuts
            .remove(&id)
            .map(|value| value.trim().to_string())
            .unwrap_or(default_keys);
        let keys = match (id.as_str(), keys.as_str()) {
            ("translate-selection", "Alt+T") => "Alt+D".to_string(),
            ("translate-screenshot", "Alt+O") => "Alt+E".to_string(),
            _ => keys,
        };
        normalized.insert(id, keys);
    }

    normalized
}
```

把 `AppConfig::from_env()` 里的内联 `HashMap::from([...])` 替换成：

```rust
shortcuts: default_shortcuts(),
```

把 `AppConfig::normalized()` 的快捷键处理替换为：

```rust
self.shortcuts = normalize_shortcuts(self.shortcuts);
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test core::config::types::tests
```

预期：`core::config::types::tests` 全部 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "fix(shortcut): 迁移默认快捷键配置"
```

---

### 任务 2：全局快捷键改为按下触发

**文件：**
- 修改：`src-tauri/src/app/shortcuts.rs:162-193`
- 测试：`src-tauri/src/app/shortcuts.rs:249-323`

- [ ] **步骤 1：编写失败的最小测试**

当前 `handle_global_shortcut()` 依赖 Tauri `AppHandle`，不为事件分支硬造测试夹具。新增一个只判断状态的小函数测试，在 `classify_shortcut` 现有测试后追加：

```rust
fn handles_pressed_shortcut_events_only() {
    assert!(should_handle_shortcut_state(ShortcutState::Pressed));
    assert!(!should_handle_shortcut_state(ShortcutState::Released));
}
```

- [ ] **步骤 2：运行快捷键测试**

运行：

```bash
cd src-tauri && cargo test app::shortcuts::tests
```

预期：FAIL，报错找不到 `should_handle_shortcut_state`。

- [ ] **步骤 3：编写最少实现代码**

在 `classify_shortcut()` 后增加：

```rust
fn should_handle_shortcut_state(state: ShortcutState) -> bool {
    state == ShortcutState::Pressed
}
```

把 `handle_global_shortcut()` 的事件过滤改为：

```rust
if !should_handle_shortcut_state(event.state) {
    return;
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
cd src-tauri && cargo test app::shortcuts::tests
```

预期：全部 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/shortcuts.rs
git commit -m "fix(shortcut): 按下时触发全局快捷键"
```

---

### 任务 3：前端保存时投影快捷键配置

**文件：**
- 修改：`frontend/src/lib/config.ts:35-54`
- 测试：`frontend/src/lib/config.test.ts:58-99`

- [ ] **步骤 1：编写失败的 Vitest 单测**

在 `describe('projectToAppConfig')` 内追加：

```ts
it('投影快捷键绑定到后端 shortcuts 并保留空字符串', () => {
  const state = makeState([]);
  state.shortcut.bindings = [
    { id: 'translate-selection', label: '划词翻译', description: '', keys: ' Alt+D ' },
    { id: 'translate-screenshot', label: '截图翻译', description: '', keys: 'Alt+E' },
    { id: 'word-lookup', label: '取词翻译', description: '', keys: '' },
  ];

  const config = projectToAppConfig(state);

  expect(config.shortcuts).toEqual({
    'translate-selection': 'Alt+D',
    'translate-screenshot': 'Alt+E',
    'word-lookup': '',
  });
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：新增测试 FAIL，当前 `shortcuts` 为 `{}`。

- [ ] **步骤 3：编写最少实现代码**

把 `projectToAppConfig()` 返回值里的 `shortcuts: {}` 替换为：

```ts
shortcuts: Object.fromEntries(
  state.shortcut.bindings.map((binding) => [binding.id, binding.keys.trim()]),
),
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：`frontend/src/lib/config.test.ts` 全部 PASS。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/lib/config.ts frontend/src/lib/config.test.ts
git commit -m "fix(settings): 保存快捷键配置"
```

---

### 任务 4：前端默认值与后端快捷键同步

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts:152-229,463-484`
- 测试：`frontend/src/settings/stores/settings.test.ts:36-47,159-240`

- [ ] **步骤 1：编写失败的 Vitest 单测**

在 `describe('settings defaults')` 内追加：

```ts
it('默认快捷键使用 Alt+D 和 Alt+E', () => {
  const { state } = useSettings();

  expect(Object.fromEntries(state.shortcut.bindings.map((b) => [b.id, b.keys]))).toMatchObject({
    'translate-selection': 'Alt+D',
    'translate-screenshot': 'Alt+E',
  });
});
```

在 `describe('syncFromBackend')` 内追加：

```ts
it('后端非空时把 shortcuts 合并回本地绑定，只覆盖 keys', async () => {
  vi.mocked(isTauriReady).mockReturnValue(true);
  const settings = useSettings();
  const localId = settings.state.services[0].id;
  const before = settings.state.shortcut.bindings.find((b) => b.id === 'translate-selection')!;

  vi.mocked(invokeGetAppConfig).mockResolvedValue({
    targetLang: '中文',
    services: [
      {
        id: localId,
        serviceType: 'deepseek',
        name: 'DeepSeek',
        enabled: false,
        protocol: 'openai_chat',
        apiKey: null,
        endpoint: 'https://api.deepseek.com',
        model: 'deepseek-chat',
        timeoutSeconds: 60,
      },
    ],
    popupPrecreate: true,
    overlayPrecreate: true,
    collectUsage: true,
    shortcuts: {
      'translate-selection': 'Ctrl+Alt+D',
      'translate-screenshot': '',
    },
  });

  await settings.syncFromBackend();

  const byId = Object.fromEntries(settings.state.shortcut.bindings.map((b) => [b.id, b]));
  expect(byId['translate-selection'].keys).toBe('Ctrl+Alt+D');
  expect(byId['translate-selection'].label).toBe(before.label);
  expect(byId['translate-screenshot'].keys).toBe('');
  expect(invokeSaveAppConfig).not.toHaveBeenCalled();
});
```

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
npm run test -- frontend/src/settings/stores/settings.test.ts
```

预期：默认快捷键测试 FAIL，后端同步测试 FAIL。

- [ ] **步骤 3：编写最少实现代码**

把 `buildDefaults()` 里的两处默认值改为：

```ts
keys: 'Alt+D',
```

```ts
keys: 'Alt+E',
```

在 `mergeBackendIntoServices()` 后新增：

```ts
const mergeBackendIntoShortcuts = (
  local: AppSettings['shortcut']['bindings'],
  backend: AppConfig['shortcuts'],
): AppSettings['shortcut']['bindings'] =>
  local.map((binding) => ({
    ...binding,
    keys: Object.prototype.hasOwnProperty.call(backend, binding.id)
      ? backend[binding.id]
      : binding.keys,
    error: undefined,
  }))
```

在 `syncFromBackend()` 的后端非空分支里，把服务合并后面改为：

```ts
state.services = mergeBackendIntoServices(state.services, backend.services)
state.shortcut.bindings = mergeBackendIntoShortcuts(state.shortcut.bindings, backend.shortcuts ?? {})
Object.assign(baseline, JSON.parse(JSON.stringify(state)))
dirty.value = false
```

- [ ] **步骤 4：运行测试验证通过**

运行：

```bash
npm run test -- frontend/src/settings/stores/settings.test.ts
```

预期：`frontend/src/settings/stores/settings.test.ts` 全部 PASS。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "fix(settings): 同步后端快捷键配置"
```

---

### 任务 5：更新窗口与托盘复用图标

**文件：**
- 修改：`src-tauri/tauri.conf.json`
- 修改：`src-tauri/icons/icon.ico`
- 临时创建后删除：`.superpowers/tmp/generate-shizi-icon.ps1`
- 测试：`src-tauri/icons/icon.ico`

- [ ] **步骤 1：生成图标**

创建临时脚本 `.superpowers/tmp/generate-shizi-icon.ps1`：

```powershell
Add-Type -AssemblyName System.Drawing

$out = Resolve-Path "src-tauri/icons/icon.ico"
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$pngs = @()

foreach ($size in $sizes) {
  $bmp = [System.Drawing.Bitmap]::new($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.Clear([System.Drawing.Color]::Transparent)

  $rect = [System.Drawing.RectangleF]::new(1, 1, $size - 2, $size - 2)
  $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
  $r = [Math]::Max(3, [int]($size * 0.18))
  $path.AddArc($rect.X, $rect.Y, $r, $r, 180, 90)
  $path.AddArc($rect.Right - $r, $rect.Y, $r, $r, 270, 90)
  $path.AddArc($rect.Right - $r, $rect.Bottom - $r, $r, $r, 0, 90)
  $path.AddArc($rect.X, $rect.Bottom - $r, $r, $r, 90, 90)
  $path.CloseFigure()
  $g.FillPath([System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(255, 15, 23, 42)), $path)

  $fontSize = [Math]::Max(8, [int]($size * 0.36))
  $font = [System.Drawing.Font]::new("Microsoft YaHei UI", $fontSize, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
  $format = [System.Drawing.StringFormat]::new()
  $format.Alignment = [System.Drawing.StringAlignment]::Center
  $format.LineAlignment = [System.Drawing.StringAlignment]::Center
  $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
  $g.DrawString("文A", $font, [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White), $rect, $format)

  $png = ".superpowers/tmp/icon-$size.png"
  $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
  $pngs += Resolve-Path $png
  $g.Dispose()
  $bmp.Dispose()
}

$fs = [System.IO.File]::Create($out)
$bw = [System.IO.BinaryWriter]::new($fs)
$bw.Write([UInt16]0)
$bw.Write([UInt16]1)
$bw.Write([UInt16]$pngs.Count)

$offset = 6 + (16 * $pngs.Count)
$data = @()
foreach ($png in $pngs) {
  $bytes = [System.IO.File]::ReadAllBytes($png)
  $data += ,$bytes
  $size = [int]([System.IO.Path]::GetFileNameWithoutExtension($png) -replace "icon-", "")
  $bw.Write([byte]($(if ($size -eq 256) { 0 } else { $size })))
  $bw.Write([byte]($(if ($size -eq 256) { 0 } else { $size })))
  $bw.Write([byte]0)
  $bw.Write([byte]0)
  $bw.Write([UInt16]1)
  $bw.Write([UInt16]32)
  $bw.Write([UInt32]$bytes.Length)
  $bw.Write([UInt32]$offset)
  $offset += $bytes.Length
}

foreach ($bytes in $data) {
  $bw.Write($bytes)
}

$bw.Dispose()
$fs.Dispose()
```

运行：

```powershell
New-Item -ItemType Directory -Force .superpowers/tmp
powershell -ExecutionPolicy Bypass -File .superpowers/tmp/generate-shizi-icon.ps1
Remove-Item .superpowers/tmp/generate-shizi-icon.ps1
Remove-Item .superpowers/tmp/icon-*.png
```

- [ ] **步骤 2：验证图标文件存在且非空**

运行：

```powershell
Get-Item src-tauri/icons/icon.ico | Select-Object Name,Length
```

预期：`icon.ico` 存在，`Length` 大于 `0`。

- [ ] **步骤 3：确保 Tauri bundle 使用该图标**

在 `src-tauri/tauri.conf.json` 中确认：

```json
"bundle": {
  "icon": [
    "icons/icon.ico"
  ]
}
```

托盘无需额外分支：`src-tauri/src/app/tray.rs` 已使用 `app.default_window_icon()`。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/icons/icon.ico
git commit -m "style(app): 更新窗口与托盘应用图标"
```

---

### 任务 6：文档同步与整体验证

**文件：**
- 修改：`README.md`（如存在）
- 修改：`AGENTS.md:67`
- 修改：`CLAUDE.md:71`
- 可能修改：含旧快捷键说明的现行文档

- [ ] **步骤 1：同步当前说明文档**

把 `AGENTS.md` 和 `CLAUDE.md` 的全局快捷键说明改成：

```markdown
- **全局快捷键**：`Alt+D` 划词复制并自动翻译；`Alt+E` 触发截图 OCR 翻译（DXGI 抓光标所在显示器整屏帧 → 自建 overlay 区域框选 → crop → Windows.Media.Ocr → 复用翻译链路）。由 `tauri-plugin-global-shortcut` 注册，逻辑集中在 `src-tauri/src/app/shortcuts.rs`。新增快捷键时需在 `capabilities/default.json` 同步授权。
```

如仓库存在 `README.md` 且包含旧默认快捷键，同步改为 `Alt+D` / `Alt+E`。不要改历史 spec/plan 里的旧快捷键记录。

- [ ] **步骤 2：运行完整验证**

运行：

```bash
npm run test
npm run typecheck
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：全部 PASS / build 成功。

- [ ] **步骤 3：检查未误改参考实现和旧 worktree**

运行：

```bash
git status --short
```

预期：改动只包含本计划列出的当前工作区文件；不包含 `pot-desktop/` 和 `.worktrees/shortcut-binding/`。

- [ ] **步骤 4：Commit**

```bash
git add README.md AGENTS.md CLAUDE.md
git commit -m "docs(shortcut): 同步默认快捷键说明"
```

如不存在 `README.md` 或没有发生改动，只提交 `AGENTS.md CLAUDE.md`：

```bash
git add AGENTS.md CLAUDE.md
git commit -m "docs(shortcut): 同步默认快捷键说明"
```

---

## 手动验收

- [ ] Windows 上启动应用，确认未自定义旧默认配置显示为 `Alt+D` / `Alt+E`。
- [ ] Chrome 聚焦网页后按 `Alt+D`，确认划词翻译触发。
- [ ] 按 `Alt+E`，确认截图 OCR overlay 打开。
- [ ] 在设置页改快捷键并保存，确认新键立即生效，旧键失效。
- [ ] 重启后确认自定义快捷键保持不变。
- [ ] 查看 Windows 窗口左上角和系统托盘，确认都是 `文A` 翻译语义图标。

## 自检

- 规格覆盖：计划覆盖 `Pressed` 事件、默认值迁移、前后端 `shortcuts` 保存和同步、窗口/托盘应用图标、文档同步。
- 占位符扫描：无未填内容或泛化执行项。
- 类型一致性：前端继续使用 `AppConfig['shortcuts']` 与 `AppSettings['shortcut']['bindings']`，后端继续使用 `HashMap<String, String>`，不新增协议结构。
