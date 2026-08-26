// 与后端 src-tauri/src/core/config/types.rs 的 AppConfig 对齐。
// 后端用 #[serde(rename_all = "camelCase")]，故前端字段全部 camelCase。
// 任何一方增删字段，必须同步本文件与 spec、README 配置说明。

export type LogLevel = 'error' | 'warn' | 'info' | 'debug';
export type ServiceProtocolId = 'openai_chat' | 'claude_messages' | 'microsoft_edge' | 'mock';
export type ChainOfThought = 'off' | 'short' | 'medium' | 'long';

export interface ServiceInstanceConfig {
  id: string;
  serviceType: string;
  name: string;
  enabled: boolean;
  protocol: ServiceProtocolId;
  apiKey: string | null;
  endpoint: string;
  model: string;
  timeoutSeconds: number;
  systemPrompt: string;
  translationPrompt: string;
  reflectionPrompt: string;
  reflectionEnabled: boolean;
  chainOfThought: ChainOfThought;
}

export interface OcrServiceInstanceConfig {
  id: string;
  serviceType: string;
  name: string;
  enabled: boolean;
  apiKey: string | null;
  endpoint: string;
  model: string;
  preferredLang: string;
  ocrPrompt: string;
}

/** 某一启动路径下翻译窗 / overlay 是否启动时预建。 */
export interface WindowPrecreatePair {
  popup: boolean;
  overlay: boolean;
}

/** 按手动启动 / 开机自启区分的预创建策略（设置 UI 不暴露）。 */
export interface WindowPrecreateConfig {
  manual: WindowPrecreatePair;
  autostart: WindowPrecreatePair;
}

export const DEFAULT_WINDOW_PRECREATE: WindowPrecreateConfig = {
  manual: { popup: true, overlay: false },
  autostart: { popup: false, overlay: false },
};

/** WebDAV 连接与远端备份目录（密码仅本机；连接状态不持久化）。 */
export interface WebDavConfig {
  url: string;
  username: string;
  password: string;
  /** 远端备份目录，如 `/shizi/`。 */
  remotePath: string;
  lastTestedAt: string;
  lastBackupAt: string;
}

/** 备份与同步（WebDAV + 本机导入/导出共用开关）。 */
export interface BackupConfig {
  webdav: WebDavConfig;
  autoSync: boolean;
  includeHistory: boolean;
  /** 默认 true：备份/导出默认包含 API Key。 */
  includeApiKeys: boolean;
}

export const DEFAULT_BACKUP_CONFIG: BackupConfig = {
  webdav: {
    url: '',
    username: '',
    password: '',
    remotePath: '/shizi/',
    lastTestedAt: '',
    lastBackupAt: '',
  },
  autoSync: false,
  includeHistory: false,
  includeApiKeys: true,
};

export interface AppConfig {
  interfaceLanguage: string;
  targetLang: string;
  defaultSourceLang: string;
  autoCopy: boolean;
  restoreClipboard: boolean;
  /** 翻译结果是否 Markdown 渲染；旧 config 可能缺省，按 true 处理。 */
  markdownRender?: boolean;
  /** 弹窗「去除空行」偏好；旧 config 可能缺省，按 false 处理。 */
  removeBlankLines?: boolean;
  /** 弹窗工具栏是否显示关闭按钮；旧 config 可能缺省，按 true 处理。 */
  showCloseButton?: boolean;
  /** 翻译弹窗是否显示在任务栏；旧 config 可能缺省，按 false 处理。 */
  showInTaskbar?: boolean;
  historyLimit: number;
  services: ServiceInstanceConfig[];
  ocrServices: OcrServiceInstanceConfig[];
  windowPrecreate: WindowPrecreateConfig;
  collectUsage: boolean;
  logLevel: LogLevel;
  updateChannel: 'stable' | 'nightly';
  autoCheckUpdate: boolean;
  /** 登录系统后自动启动（Windows Run 键）。旧 config 可能缺省。 */
  launchAtLogin?: boolean;
  /** WebDAV 备份；旧 config 可能缺省。 */
  backup?: BackupConfig;
  shortcuts: Record<string, string>;
}
