# Agent Team 任务看板

> 当前 plan：2026-07-08 翻译弹窗语言联动与卡片视觉优化
> PM：用户本人（编排执行：Claude 主会话）
> 规范：docs/agent-team.md
> spec：docs/superpowers/specs/2026-07-08-translation-popup-language-and-visual-design.md
> 主 plan：docs/superpowers/plans/2026-07-08-translation-popup-language-and-visual.md
> 后端分册：docs/agent/2026-07-08-translation-popup-language-and-visual-backend.md
> 前端分册：docs/agent/2026-07-08-translation-popup-language-and-visual-frontend.md

## 任务状态

| task_id | 角色 | 描述 | 状态 | model_tier | depends_on |
|---|---|---|---|---|---|
| ARCH-01 | 架构师 | 产出主 plan + 前后端分册 | done | strong(opus) | - |
| BE-1 | 后端 | AppState 会话语言字段+方法+初始化（TDD） | pending | weak(sonnet) | - |
| BE-2 | 后端 | DEFAULT_TARGET_LANG 改 zh-CN + 测试 | pending | weak(sonnet) | - |
| BE-3 | 后端 | SessionLanguages DTO + get/set command + 翻译入口 | pending | weak(sonnet) | BE-1 |
| BE-4 | 后端 | lib.rs 注册两个 command | pending | weak(sonnet) | BE-3 |
| BE-5 | 后端 | tauri.conf.json main 加 skipTaskbar | pending | weak(sonnet) | - |
| FE-1 | 前端 | settings.ts defaultTargetLang + syncFromBackend 回读 | pending | weak(sonnet) | - |
| FE-2 | 前端 | TranslatePanel.vue 源补 auto + 目标过滤 auto | pending | weak(sonnet) | - |
| FE-3 | 前端 | translate.js 下拉+会话语言+蓝点+card-sync 清理 | pending | weak(sonnet) | - |
| REV-1 | Review | Spec Reviewer（含 UI 无关性校验） | pending | weak 先跑 | 实现全完成 |
| REV-2 | Review | Quality Reviewer | pending | weak 先跑 | REV-1 通过 |

## DAG
- 串行链：BE-1 -> BE-3 -> BE-4
- 可并行：BE-2、BE-5、FE-1、FE-2、FE-3（files_to_write 无交集）
- Review：实现全完成 -> REV-1 -> REV-2

## 文件锁
见 docs/agent/locks.json（实现 dispatch 时按 task_id 锁 files_to_write）
