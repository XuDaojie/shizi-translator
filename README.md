# 柿子翻译（Shizi）

[![License: GPL-3.0-only](https://img.shields.io/badge/license-GPL--3.0--only-orange?logo=gnu)](LICENSE)
[![Latest Release](https://img.shields.io/github/v/release/XuDaojie/shizi-translator?include_prereleases)](https://github.com/XuDaojie/shizi-translator/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/XuDaojie/shizi-translator/total)](https://github.com/XuDaojie/shizi-translator/releases)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![Windows](https://img.shields.io/badge/Windows-0078D4?logo=windows11&logoColor=white)](https://www.microsoft.com/windows/)

<p align="center">
  <img src="src-tauri/icons/icon-1024.png" alt="柿子翻译 Logo" width="96" />
</p>

**柿子翻译**（拼音 **Shizi**，简称 **柿子**）是一款开源的 Windows 桌面翻译软件，支持输入翻译、划词翻译与截图 OCR 翻译，可接入 OpenAI 兼容接口、Claude、Microsoft Edge 等翻译服务。

灵感来自 macOS 平台的 [Bob](https://bobtranslate.com/)，基于 [Tauri 2](https://tauri.app/)、Rust 与 Vue 3 构建。

> Shizi is an open-source Windows translator powered by LLMs and OCR — selection translate, screenshot OCR, and multi-service streaming results.

## 演示

<table>
  <tr>
    <td align="center" width="33%" valign="middle">
      <!-- 输入翻译演示 GIF 待补充 -->
      <br /><br /><sub>（演示待补充）</sub><br /><br />
    </td>
    <td align="center" width="33%" valign="middle">
      <img src="asserts/2.gif" alt="截图翻译演示" width="100%" />
    </td>
    <td align="center" width="33%" valign="middle">
      <img src="asserts/1.gif" alt="划词翻译演示" width="100%" />
    </td>
  </tr>
  <tr>
    <td align="center"><b>输入翻译</b></td>
    <td align="center"><b>截图翻译</b></td>
    <td align="center"><b>划词翻译</b></td>
  </tr>
  <tr>
    <td align="center"><sub>启动应用 · 手动输入</sub></td>
    <td align="center"><code>Alt+S</code></td>
    <td align="center"><code>Alt+D</code></td>
  </tr>
</table>

## 下载安装

1. 打开 [Releases](https://github.com/XuDaojie/shizi-translator/releases) 页面。
2. 下载最新 Windows 安装包（NSIS，文件名类似 `Shizi_x.y.z_x64-setup.exe`）。
3. 安装后从开始菜单启动，或使用系统托盘菜单。

当前版本可能为 beta：预览版也会出现在 Releases 中（标题或 tag 含 `beta`）。

### 系统要求

- Windows 10 / 11（x64）
- [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（Windows 11 通常已预装）
- 截图 OCR 使用系统 Windows OCR 时，需安装对应语言的 OCR 语言包（中英混合场景尤其需要）

## 快速开始

1. **启动应用**，默认显示翻译弹窗。
2. **配置服务**（设置 → 服务）：翻译渠道与文字识别（OCR）分开配置，见下方「推荐配置」。
3. **开始翻译**：
   - **输入翻译**：在弹窗输入文本，点击「翻译」。
   - **划词翻译**：选中文本后按 `Alt+D`。
   - **截图翻译**：按 `Alt+S` 框选屏幕区域，识别文字后自动翻译。

默认快捷键可在设置页「快捷键」中修改：

| 功能 | 默认快捷键 |
|------|------------|
| 划词翻译 | `Alt+D` |
| 截图翻译 | `Alt+S` |
| 文字识别（仅识别，不翻译） | `Alt+O` |
| 剪贴板翻译 | `Ctrl+Shift+C` |

## 推荐配置（免费 / 低成本）

应用**内置**可零配置跑通的路径，也支持接入各家云 API（通常比系统自带能力更好用，但需自行注册并填写 API Key）。

> **关于「免费」：** 下文提到的模型，是我们在**当前**各平台政策下实测可用、且标为免费（或等价零费用额度）的推荐。  
> **不保证永远免费**——供应商随时可能调整价格、额度或下线模型。请以各平台控制台 / 价目表为准；若下方 id 在控制台不可用，可换同平台其他免费或低价模型。

### 翻译

| 路径 | 怎么配 | 说明 |
|------|--------|------|
| **零配置** | 设置 → 服务 → 启用「微软翻译」 | Edge 引擎机器翻译，**无需 API Key**，适合立刻验证整条链路 |
| **硅基流动（当前免费文本模型）** | 添加「硅基流动」，模型填 `Qwen/Qwen3-32B` | 需在[硅基流动](https://cloud.siliconflow.cn/account/ak)申请 Key；同平台往往还有其他免费文本模型，我们当前主要用这一款 |
| **智谱 AI 等** | 添加「智谱 AI」等渠道，在控制台选用当前免费 / 低价文本模型 | 需 API Key；具体免费模型以智谱控制台为准 |

也可同时启用多个服务，在弹窗里对比机器翻译与大模型结果。

### 文字识别（OCR）

截图翻译与独立文字识别窗口共用**当前启用的一个** OCR 引擎（设置 → 服务 → 文字识别）。

| 路径 | 怎么配 | 说明 |
|------|--------|------|
| **系统内置** | 「Windows 媒体 OCR」 | **无需 Key**，本机识别；依赖系统 OCR 语言包，复杂排版 / 小字 / 多语言混排时效果有限 |
| **硅基流动视觉 OCR（当前免费）** | 添加「硅基流动」视觉渠道，模型 `deepseek-ai/DeepSeek-OCR` | 需 API Key；多模态识图通常明显优于系统 OCR，适合截图翻译提质 |
| **智谱视觉（当前免费）** | 添加「智谱 AI」视觉渠道，模型 `glm-4v-flash` | 需在[智谱开放平台](https://open.bigmodel.cn/usercenter/apikeys)申请 Key；同样走云端多模态识图 |

> 提示：Windows OCR 与视觉 OCR **互斥**，同一时间只有一个引擎生效。想提升截图翻译质量时，在「文字识别」里启用硅基流动或智谱视觉即可，无需改翻译渠道。

### 建议组合（示例）

| 目标 | 翻译 | OCR |
|------|------|-----|
| 立刻能用、零 Key | 微软翻译 | Windows 媒体 OCR |
| 当前政策下免费、效果更好 | 硅基流动 `Qwen/Qwen3-32B` | 硅基流动 `deepseek-ai/DeepSeek-OCR`，或智谱 `glm-4v-flash` |
| 多服务对比 | 微软翻译 + 硅基流动等 | 按需选系统或视觉 OCR |

## 核心功能

### 输入翻译

启动后在翻译弹窗输入原文并翻译。适合主动输入、粘贴长文或对照多服务结果。

### 截图翻译

1. 按 `Alt+S`，当前屏幕冻结为框选界面。
2. 拖动选择要识别的区域。
3. 松开后，当前启用的 OCR 引擎识别文字并进入翻译。

> Esc、右键或选区过小（&lt;3px）会取消本次截图。Windows OCR 依赖系统语言包。

### 划词翻译

1. 在任意支持复制的应用中选中文本。
2. 按 `Alt+D`。
3. 应用读取选中文本并自动翻译。

> 划词复制主要保护纯文本剪贴板，不保证完整恢复图片、文件、HTML、RTF 等非文本格式。

### 独立文字识别

托盘「文字识别」或 `Alt+O` 打开识别窗口，支持截图框选、打开图片、读取剪贴板图片。结果仅展示识别文本与元信息，**不**自动翻译、不写入翻译历史。

## 主要特性

- **多服务并行**：可同时启用多个翻译渠道，按列表顺序展示多张结果卡；单服务失败不影响其他服务。
- **流式输出**：大模型结果边生成边显示，支持取消与失败后重试。
- **免 Key 机器翻译**：内置微软翻译（Edge 引擎），便于零配置试用。
- **主流大模型协议**：OpenAI 兼容 Chat Completions、Claude Messages 等。
- **托盘常驻**：关闭窗口默认隐藏到托盘，从托盘退出应用。
- **可配置快捷键**：划词 / 截图 / 文字识别 / 剪贴板等绑定可改、可清空，保存后即时生效。
- **翻译历史**：手动、划词、截图翻译按批次写入本机 SQLite，可在设置页查看与清空。
- **界面多语言**：内置简中 / 繁中 / 英 / 日 / 韩 / 法 / 德 / 西等界面语言；`auto` 跟随系统。
- **源语言自动检测**：选「自动检测」时，译文区显示检测到的语言。
- **OCR 双引擎**：系统 Windows.Media.Ocr，或 OpenAI 兼容视觉模型；截图翻译与独立识别共用当前启用的引擎。
- **日志与排障**：前后端独立日志、等级可调、敏感信息脱敏，支持一键导出诊断包。

## 配置说明

设置页为独立窗口，主要分类：

| 分类 | 内容 |
|------|------|
| 通用 | 开机启动、界面语言、关闭行为等 |
| 翻译 | 源 / 目标语言、复制粘贴相关行为 |
| 快捷键 | 全局快捷键绑定 |
| 服务 | 翻译渠道与文字识别（OCR）实例 |
| 历史 | 最近翻译记录 |
| 高级 | 日志等级、导出日志、配置导入导出、重置、关于 |

### 首次使用建议

1. 先启用 **微软翻译** + **Windows 媒体 OCR**，零 Key 验证整条链路。
2. 再按「推荐配置」接入当前免费模型（如硅基流动 `Qwen/Qwen3-32B` + `deepseek-ai/DeepSeek-OCR`），提升翻译与截图识别质量。
3. 截图识别不准时：先查 Windows OCR 语言包；需要更好效果则改用视觉 OCR（如 `DeepSeek-OCR` 或 `glm-4v-flash`）。

### 数据与安全

- 配置保存在本机应用配置目录下的 `config.json`。
- 翻译历史保存在同目录的 `history.sqlite3`。
- **API Key 当前以明文写入本地配置文件**。请勿在不受信任的环境中填入密钥；后续版本计划迁移到系统凭据存储。

用户界面语言包可放在 `<app_config_dir>/lang/*.json`，在设置页「通用」中打开目录并刷新，无需重启。仅允许覆盖内置消息 key，不能新增未知 key。

## 系统要求与已知限制

### 平台

- 当前面向 **Windows** 优先；安装包为 Windows NSIS。
- 部分能力依赖 Windows 系统 API（DXGI 截图、Windows OCR 等）。

### 功能限制

- 取词翻译（悬停取词等）、快捷键分组 / profile 等尚未实现。
- API Key 明文保存在 `config.json`（见上文安全说明）。
- 用户语言包只能覆盖内置文案 key。

### 截图 OCR

- 多显示器下，抓帧按光标定位显示器，但框选窗口在部分场景可能落在主屏，副屏使用时可能错位。
- 混合 DPI 多屏下，框选坐标可能不够精确。
- 锁屏 / 屏保 / 安全桌面 / 远程会话中，截图可能失败。

## 从源码构建

适合想本地调试或自行打包的开发者。

### 环境

- Node.js（用于前端与 `@tauri-apps/cli`）
- Rust stable（edition 2021）
- Windows + WebView2 Runtime

### 常用命令

```bash
npm install                 # 安装依赖
npm run tauri dev           # 开发模式（Vite + 后端）
npm run tauri build         # 生成 release 安装包（NSIS）
npm run typecheck           # 前端类型检查
npm run test                # 前端单测
cd src-tauri && cargo test  # 后端单测
```

仅启动前端（无 Tauri 容器，`invoke` 不可用）：

```bash
npm run dev
```

更细的模块结构与协作约定见仓库内 [`AGENTS.md`](AGENTS.md) 与 [`docs/`](docs/)。

## 贡献

欢迎通过 [Issues](https://github.com/XuDaojie/shizi-translator/issues) 反馈问题与建议，或提交 Pull Request。

提交前建议本地跑通：

```bash
npm run typecheck
npm run test
cd src-tauri && cargo test
```

## 许可证

除另有说明外，本项目源代码依据 [GNU General Public License v3.0](LICENSE) 仅第 3 版（`GPL-3.0-only`）发布。分发修改版本时，须按许可证提供对应源代码。

`Shizi`（柿子拼音）/ 柿子 / 柿子翻译 名称与 Logo 的商标权及相关品牌权利不随 GPL 授权。

## 致谢

- [Bob](https://bobtranslate.com/) — 交互与产品形态灵感
- [Tauri](https://tauri.app/) — 桌面应用框架
- 以及所有开源依赖的作者与贡献者
