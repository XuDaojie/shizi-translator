# WebDAV 备份与同步

> 日期：2026-08-05  
> 状态：已批准，实现中  
> 规模：M  
> 原型源：`OpenDesignProjects/shizi`（`AdvancedPanel.vue` / `settings-advanced-backup.html`）

## 1. 背景与目标

设置 → 高级 增加 **备份与同步 · WebDAV**（主路径），保留并强化 **本机导入/导出**（兜底）。对齐高保真原型：测试连接、立即备份、从云端选包恢复、自动备份、含历史 / 含 API Key。

## 2. 已确认决策

| 项 | 决策 |
|----|------|
| 规模 | M：本对话实现，不写 formal plan、不跨对话 L 交接 |
| 实现深度 | 真 WebDAV（Basic + PROPFIND / MKCOL / PUT / GET），非 mock |
| 远端格式 | 目录内多份 `shizi-backup-YYYYMMDD-HHMMSS.zip` |
| 本机导出 | 与远端同 schema 的 JSON 包装（`kind: shizi-settings` / zip 内同文件集）；本机以 JSON 文件为主便于编辑 |
| `includeHistory` 默认 | `false` |
| **`includeApiKeys` 默认** | **`true`（默认勾选）** |
| 恢复历史策略 | 备份含历史时 **整库替换**，不做按 id 合并 |
| status / lastError | 仅 UI 运行时，不持久化 |
| 自动备份 | 配置保存成功后防抖约 30s 仅上传；恢复一律手动 |
| 密码 | 随 `config.json` 本机保存；导出/备份在 `includeApiKeys=false` 时剥离 |

### UI 行顺序（WebDAV 组内）

1. 连接信息（URL / 用户名 / 密码 / 远程目录 + 测试连接）  
2. **包含翻译历史**  
3. **包含 API Key**  
4. **手动同步**（立即备份 / 从云端恢复 + 上次备份时间）  
5. **自动备份**

说明：选项在前，最终操作在后（用户确认调整，2026-08-05）。

## 3. 范围

### 做

1. `AppConfig.backup` 持久化字段 + 前后端同步  
2. Rust `core/backup`：快照组包、WebDAV 客户端、列表解析  
3. Commands：`test_webdav_connection`、`backup_to_webdav`、`list_webdav_backups`、`restore_from_webdav`；本机导出/导入经前端 + 历史 list/replace  
4. `HistoryStore` 支持整库导出/替换导入  
5. 设置页 AdvancedPanel 对齐原型（i18n、确认对话框）  
6. 自动备份 debounce  
7. 单元测试（路径规范化、Basic、PROPFIND 解析、manifest、config 默认）

### 不做（YAGNI）

- S3 / OneDrive / 非 WebDAV  
- 本地定时备份库、版本 diff、加密容器  
- 恢复时历史按 id 合并  
- 多用户冲突解决  

## 4. 数据模型

```text
AppConfig.backup:
  webdav:
    url, username, password, remotePath  # 目录，默认 /shizi/backups/
    lastTestedAt, lastBackupAt           # ISO 或空串
  autoSync: bool                         # 默认 false
  includeHistory: bool                   # 默认 false
  includeApiKeys: bool                   # 默认 true
```

快照 zip / JSON：

- `manifest.json`：`version: 1`、`kind: "shizi-backup"`、`exportedAt`、`includeHistory`、`includeApiKeys`
- `settings.json`：`AppConfig` 投影（可按开关剥离 `apiKey` 与 webdav password）
- 可选 `history.json`：`HistorySessionDto[]`

## 5. 命令与错误

| Command | 行为 |
|---------|------|
| `test_webdav_connection` | 校验 http(s)；Basic；对目录 PROPFIND；成功写 `lastTestedAt` |
| `backup_to_webdav` | 组包 → MKCOL → PUT → 写 `lastBackupAt` |
| `list_webdav_backups` | PROPFIND Depth:1，筛 `shizi-backup-*.zip` |
| `restore_from_webdav` | GET → 解压 → 覆盖 config；含历史则整库替换 |

错误以中文 `String` 返回前端 toast；网络/鉴权失败可读。

## 6. 测试

- 路径规范化（文件路径回退目录、补 `/`）  
- manifest 读写与 Key 剥离  
- PROPFIND 列表解析  
- `includeApiKeys` 默认 true  
- 前端 config-io / 默认 backup 结构  

## 7. 文档同步（收尾）

- `docs/agent/architecture-notes.md` 配置与命令摘要  
- 必要时 README 一句能力说明  
- 本 spec 状态改为已实现  
