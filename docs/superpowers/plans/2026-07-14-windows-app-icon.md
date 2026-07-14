# Shizi Windows 桌面图标实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 生成并集成已确认的 Shizi Windows 图标，保留完整构图与小尺寸构图两份 SVG，并产出包含独立 16/20/24 px 帧的 `icon.ico`。

**架构：** `icon.svg` 是 32 px 及以上构图的事实来源，`icon-small.svg` 是 16/20/24 px 无箭头构图的事实来源。Tauri CLI 直接从两份 SVG 渲染 PNG；一次性使用当前环境已有的 Pillow 将不同来源的帧组装进最终 ICO，不新增长期生成脚本或依赖。

**技术栈：** SVG、Tauri CLI 2.11.4、PowerShell、Python 3 + Pillow、Tauri 2 / NSIS

---

## 文件结构

- 创建：`src-tauri/icons/icon.svg` — 32 px 及以上完整构图源文件。
- 创建：`src-tauri/icons/icon-small.svg` — 16/20/24 px 无箭头光学校正源文件。
- 创建：`src-tauri/icons/icon-1024.png` — 完整构图 1024 px 透明 PNG。
- 修改：`src-tauri/icons/icon.ico` — Windows 多帧最终图标。
- 修改：`docs/superpowers/specs/2026-07-14-windows-app-icon-design.md` — 回填实现状态。
- 修改：`docs/superpowers/plans/2026-07-14-windows-app-icon.md` — 回填任务复选框。
- 修改：`README.md` — 记录新的 Windows 应用图标。
- 修改：`docs/roadmap/progressive-development-plan.md` — 回填任务 6 的图标子项状态。
- 验证但不修改：`src-tauri/tauri.conf.json` — 保持 `bundle.icon = ["icons/icon.ico"]`。

## 任务 1：创建两份 SVG 事实来源

**文件：**
- 创建：`src-tauri/icons/icon.svg`
- 创建：`src-tauri/icons/icon-small.svg`

- [ ] **步骤 1：运行源文件基线检查并确认失败**

运行：

```powershell
@'
from pathlib import Path
for name in ("icon.svg", "icon-small.svg"):
    path = Path("src-tauri/icons") / name
    assert path.exists(), f"missing {path}"
'@ | python -
```

预期：FAIL，提示缺少 `src-tauri/icons/icon.svg`。

- [ ] **步骤 2：创建完整构图 SVG**

使用 `apply_patch` 创建 `src-tauri/icons/icon.svg`：

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="persimmon" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#73852E"/>
      <stop offset="0.24" stop-color="#D99624"/>
      <stop offset="0.62" stop-color="#E75B25"/>
      <stop offset="1" stop-color="#BD371D"/>
    </linearGradient>
  </defs>
  <rect width="128" height="128" rx="28" fill="url(#persimmon)"/>
  <text x="7" y="78" fill="#FFF" font-family="Microsoft YaHei" font-size="62" font-weight="700">文</text>
  <text x="82" y="101" fill="#FFF" font-family="Segoe UI" font-size="49" font-weight="700">A</text>
  <path d="M64 51h25l-7-6m7 6-7 6M91 66H66l7 6m-7-6 7-6" fill="none" stroke="#FFF3DF" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
```

- [ ] **步骤 3：创建小尺寸构图 SVG**

使用 `apply_patch` 创建 `src-tauri/icons/icon-small.svg`：

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="persimmon" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#73852E"/>
      <stop offset="0.24" stop-color="#D99624"/>
      <stop offset="0.62" stop-color="#E75B25"/>
      <stop offset="1" stop-color="#BD371D"/>
    </linearGradient>
  </defs>
  <rect width="128" height="128" rx="26" fill="url(#persimmon)"/>
  <text x="3" y="84" fill="#FFF" font-family="Microsoft YaHei" font-size="71" font-weight="700">文</text>
  <text x="83" y="106" fill="#FFF" font-family="Segoe UI" font-size="56" font-weight="700">A</text>
</svg>
```

- [ ] **步骤 4：运行最小结构检查**

运行：

```powershell
@'
from pathlib import Path
from xml.etree import ElementTree as ET

root = Path("src-tauri/icons")
full = ET.parse(root / "icon.svg").getroot()
small = ET.parse(root / "icon-small.svg").getroot()
ns = {"svg": "http://www.w3.org/2000/svg"}

assert full.attrib["viewBox"] == "0 0 128 128"
assert small.attrib["viewBox"] == "0 0 128 128"
assert [node.text for node in full.findall("svg:text", ns)] == ["文", "A"]
assert [node.text for node in small.findall("svg:text", ns)] == ["文", "A"]
assert len(full.findall("svg:path", ns)) == 1
assert len(small.findall("svg:path", ns)) == 0
assert [node.attrib["stop-color"] for node in full.findall(".//svg:stop", ns)] == ["#73852E", "#D99624", "#E75B25", "#BD371D"]
print("SVG structure OK")
'@ | python -
```

预期：输出 `SVG structure OK`。

- [ ] **步骤 5：提交 SVG 源文件**

```powershell
git add src-tauri/icons/icon.svg src-tauri/icons/icon-small.svg
git commit -m "feat(icon): 添加可编辑图标源文件"
```

## 任务 2：渲染 PNG 并组装精确 ICO

**文件：**
- 创建：`src-tauri/icons/icon-1024.png`
- 修改：`src-tauri/icons/icon.ico`

- [ ] **步骤 1：确认本地工具可用**

运行：

```powershell
npm run tauri -- --version
python -c "import PIL; from PIL import Image; print(PIL.__version__)"
Get-Item "$env:WINDIR/Fonts/msyhbd.ttc", "$env:WINDIR/Fonts/segoeuib.ttf" | Select-Object Name,Length
```

预期：Tauri 输出 `tauri-cli 2.11.4`；Pillow 可导入；微软雅黑粗体与 Segoe UI 粗体字体文件均存在。

- [ ] **步骤 2：安全创建临时构建目录**

运行：

```powershell
$workspace = (Resolve-Path '.').Path
$build = Join-Path $workspace 'tmp/icon-build'
if (-not $build.StartsWith($workspace, [System.StringComparison]::OrdinalIgnoreCase)) { throw '图标临时目录越界' }
if (Test-Path -LiteralPath $build) { Remove-Item -LiteralPath $build -Recurse -Force }
New-Item -ItemType Directory -Force -Path (Join-Path $build 'main'), (Join-Path $build 'small') | Out-Null
```

预期：`tmp/icon-build/main` 与 `tmp/icon-build/small` 存在。

- [ ] **步骤 3：用 Tauri CLI 从两份 SVG 分别渲染帧**

运行：

```powershell
npm run tauri -- icon src-tauri/icons/icon.svg --output tmp/icon-build/main --png 32 --png 48 --png 64 --png 128 --png 256 --png 1024
npm run tauri -- icon src-tauri/icons/icon-small.svg --output tmp/icon-build/small --png 16 --png 20 --png 24
```

预期：主目录包含 `32x32.png`、`48x48.png`、`64x64.png`、`128x128.png`、`256x256.png`、`1024x1024.png`；小尺寸目录包含 `16x16.png`、`20x20.png`、`24x24.png`。

- [ ] **步骤 4：验证每张 PNG 的像素尺寸与透明通道**

运行：

```powershell
@'
from pathlib import Path
from PIL import Image

root = Path("tmp/icon-build")
files = {
    **{size: root / "small" / f"{size}x{size}.png" for size in (16, 20, 24)},
    **{size: root / "main" / f"{size}x{size}.png" for size in (32, 48, 64, 128, 256, 1024)},
}
for size, path in files.items():
    image = Image.open(path)
    assert image.size == (size, size), (path, image.size)
    assert image.mode == "RGBA", (path, image.mode)
    assert image.getpixel((0, 0))[3] == 0, f"corner is not transparent: {path}"
print("PNG frames OK")
'@ | python -
```

预期：输出 `PNG frames OK`。

- [ ] **步骤 5：复制 1024 PNG 并组装最终 ICO**

运行：

```powershell
Copy-Item -LiteralPath 'tmp/icon-build/main/1024x1024.png' -Destination 'src-tauri/icons/icon-1024.png' -Force
@'
from pathlib import Path
from PIL import Image

root = Path("tmp/icon-build")
paths = [
    root / "small" / f"{size}x{size}.png" for size in (16, 20, 24)
] + [
    root / "main" / f"{size}x{size}.png" for size in (32, 48, 64, 128, 256)
]
frames = [Image.open(path).convert("RGBA") for path in paths]
frames[-1].save(
    "src-tauri/icons/icon.ico",
    format="ICO",
    sizes=[frame.size for frame in frames],
    append_images=frames[:-1],
)
'@ | python -
```

预期：`icon-1024.png` 与新的 `icon.ico` 写入成功。

- [ ] **步骤 6：验证 ICO 尺寸集合及帧内容未被二次缩放**

运行：

```powershell
@'
from pathlib import Path
from PIL import Image, ImageChops

root = Path("tmp/icon-build")
expected_paths = {
    **{size: root / "small" / f"{size}x{size}.png" for size in (16, 20, 24)},
    **{size: root / "main" / f"{size}x{size}.png" for size in (32, 48, 64, 128, 256)},
}
ico = Image.open("src-tauri/icons/icon.ico")
assert set(ico.ico.sizes()) == {(size, size) for size in expected_paths}, ico.ico.sizes()
for size, path in expected_paths.items():
    expected = Image.open(path).convert("RGBA")
    actual = ico.ico.getimage((size, size)).convert("RGBA")
    assert ImageChops.difference(expected, actual).getbbox() is None, f"frame mismatch: {size}"
png = Image.open("src-tauri/icons/icon-1024.png")
assert png.size == (1024, 1024) and png.mode == "RGBA"
print("ICO frames OK")
'@ | python -
```

预期：输出 `ICO frames OK`，尺寸集合精确包含 16、20、24、32、48、64、128、256。

- [ ] **步骤 7：生成实际像素 QA 联系表并检查**

运行：

```powershell
@'
from pathlib import Path
from PIL import Image, ImageDraw

root = Path("tmp/icon-build")
sizes = (16, 20, 24, 32, 48, 64, 128, 256)
sheet = Image.new("RGB", (960, len(sizes) * 300), "#ECEFF1")
draw = ImageDraw.Draw(sheet)
for row, size in enumerate(sizes):
    source = root / ("small" if size <= 24 else "main") / f"{size}x{size}.png"
    image = Image.open(source).convert("RGBA")
    y = row * 300 + 12
    draw.text((16, y), f"{size} px", fill="#20282C")
    sheet.paste(image, (100, y), image)
    zoom = image.resize((256, 256), Image.Resampling.NEAREST)
    sheet.paste(zoom, (380, y), zoom)
    draw.rectangle((660, y, 940, y + 280), fill="#20282C")
    sheet.paste(zoom, (672, y + 12), zoom)
sheet.save(root / "icon-contact-sheet.png")
'@ | python -
```

打开 `tmp/icon-build/icon-contact-sheet.png`，确认：16/20/24 px 无箭头且「文」与 A 分离；32 px 起箭头可辨；所有尺寸圆角和字符均未裁切。

- [ ] **步骤 8：提交最终图标资产**

```powershell
git add src-tauri/icons/icon-1024.png src-tauri/icons/icon.ico
git commit -m "feat(icon): 生成 Windows 多尺寸应用图标"
```

## 任务 3：验证 Tauri 打包并同步文档

**文件：**
- 修改：`README.md`
- 修改：`docs/roadmap/progressive-development-plan.md`
- 修改：`docs/superpowers/specs/2026-07-14-windows-app-icon-design.md`
- 修改：`docs/superpowers/plans/2026-07-14-windows-app-icon.md`
- 验证：`src-tauri/tauri.conf.json`

- [ ] **步骤 1：验证 Tauri 配置仍引用最终 ICO**

运行：

```powershell
@'
import json
from pathlib import Path
config = json.loads(Path("src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
assert config["bundle"]["icon"] == ["icons/icon.ico"]
print("Tauri icon config OK")
'@ | python -
```

预期：输出 `Tauri icon config OK`。

- [ ] **步骤 2：执行真实 NSIS 打包验证**

运行：

```powershell
npm run tauri build
```

预期：退出码 0；`src-tauri/target/release/shizi.exe` 与 `src-tauri/target/release/bundle/nsis/*.exe` 生成成功。

- [ ] **步骤 3：提取应用与安装包图标进行最终目视检查**

运行：

```powershell
Add-Type -AssemblyName System.Drawing
$build = (Resolve-Path 'tmp/icon-build').Path
$app = (Resolve-Path 'src-tauri/target/release/shizi.exe').Path
$installer = (Get-ChildItem 'src-tauri/target/release/bundle/nsis/*.exe' | Select-Object -First 1).FullName
[System.Drawing.Icon]::ExtractAssociatedIcon($app).ToBitmap().Save((Join-Path $build 'app-extracted.png'))
[System.Drawing.Icon]::ExtractAssociatedIcon($installer).ToBitmap().Save((Join-Path $build 'installer-extracted.png'))
```

打开 `app-extracted.png` 与 `installer-extracted.png`，确认均为新的柿子成熟色「文 / A」图标。

- [ ] **步骤 4：同步项目文档**

在 `README.md` 的「当前能力」中新增：

```markdown
- Windows 应用图标：采用柿子成熟色「文 / A」字标，ICO 内置独立 16/20/24 px 光学校正帧，32 px 起显示双向转换箭头。
```

将 `docs/roadmap/progressive-development-plan.md` 中任务 6 更新为：

```text
任务 6：完善 Windows 安装包、图标、托盘、开机自启（应用图标已完成，其余子项继续推进）。
```

在设计规格中新增实现状态：

```markdown
## 实现状态

- [x] 完整构图与小尺寸构图 SVG 已保存。
- [x] 1024 px PNG 与多帧 ICO 已生成。
- [x] Tauri NSIS 打包及应用/安装包图标已验证。
```

将本计划已完成步骤的复选框改为 `[x]`。

- [ ] **步骤 5：运行最终验证**

运行：

```powershell
git diff --check
@'
from PIL import Image
ico = Image.open("src-tauri/icons/icon.ico")
assert set(ico.ico.sizes()) == {(s, s) for s in (16, 20, 24, 32, 48, 64, 128, 256)}
assert Image.open("src-tauri/icons/icon-1024.png").size == (1024, 1024)
print("Final icon assets OK")
'@ | python -
git status --short
```

预期：`git diff --check` 无输出；脚本输出 `Final icon assets OK`；状态仅包含 README、roadmap、spec 和 plan 的预期文档修改。

- [ ] **步骤 6：提交文档同步**

```powershell
git add README.md docs/roadmap/progressive-development-plan.md docs/superpowers/specs/2026-07-14-windows-app-icon-design.md docs/superpowers/plans/2026-07-14-windows-app-icon.md
git commit -m "docs(icon): 回填应用图标实现状态"
```
