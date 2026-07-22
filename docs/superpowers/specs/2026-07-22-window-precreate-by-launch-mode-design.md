# 按启动路径的窗口预创建策略

## 背景

翻译窗曾写死在 `tauri.conf.json`，启动即建 WebView；`popupPrecreate` / `overlayPrecreate` 设置项与真实行为脱节。开机自启用户未必使用应用，不应默认占用 WebView 内存。

## 目标

1. 设置 UI **移除**两个预创建开关。
2. 配置按 **手动启动 / 开机自启** 拆分，无旧字段兼容。
3. 默认：自启双关；手动仅预建翻译窗（并展示），Overlay 不预建。
4. 翻译窗首次用过后关窗仍 hide 常驻；设置 / OCR 关闭即销毁（既有）。

## 配置

```json
{
  "windowPrecreate": {
    "manual": { "popup": true, "overlay": false },
    "autostart": { "popup": false, "overlay": false }
  }
}
```

- 运行时：`is_autostart_process()` → `autostart`，否则 `manual`。
- 删除 `popupPrecreate` / `overlayPrecreate`；不迁移旧 config。
- 不在设置页暴露；`save_app_config` 透传当前值（前端 sync 后原样写回）。

## 窗口生命周期

| 路径 | popup=true | popup=false |
|------|------------|-------------|
| 启动 | ensure 隐藏 main（手动启动后前端 ready show） | 不建 |
| 首次唤起 | 复用 | ensure 再建 |
| 关窗 | prevent_close + hide | 同左 |

| 路径 | overlay=true | overlay=false |
|------|--------------|---------------|
| 启动 | ensure 隐藏 overlay | 不建 |
| 首次截图 | 复用 + reload | ensure 新建；之后 hide 复用 + reload |

- `main` 从 `tauri.conf` 静态列表移除，运行时 `WebviewWindowBuilder` 创建。
- 无窗托盘驻留：`ExitRequested` 且无 exit code 时 `prevent_exit`（托盘退出仍 `app.exit`）。
- Windows 首次建窗仍避免在同步 tray/快捷键栈上直接 build（沿用独立线程 / async）。

## 非目标

- 旧配置字段兼容。
- 设置页再暴露预创建项。
- 翻译窗关即销毁。

## 验收

1. 自启：仅托盘，无 main/overlay WebView；第一次划词才建 main。
2. 手动默认：启动有 main 并 show；无 overlay。
3. 第一次截图才建 overlay；提交后 hide，再次截图 reload 复用。
4. 设置页无预创建开关；config 含 `windowPrecreate` 默认值。
