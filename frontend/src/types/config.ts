// 与后端 src-tauri/src/core/config/types.rs 的 AppConfig 对齐。
// 后端用 #[serde(rename_all = "camelCase")]，故前端字段全部 camelCase。
// 任何一方增删字段，必须同步本文件与 spec、README 配置说明。

export type Provider = 'openai-compatible' | 'claude' | 'mock';

export interface OpenAiCompatibleConfig {
  apiKey: string | null;
  baseUrl: string;
  model: string;
  timeoutSeconds: number;
}

export interface ClaudeConfig {
  apiKey: string | null;
  baseUrl: string;
  model: string;
  timeoutSeconds: number;
  enableThinking: boolean;
}

export interface AppConfig {
  provider: Provider;
  targetLang: string;
  openaiCompatible: OpenAiCompatibleConfig;
  claude: ClaudeConfig;
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
  shortcuts: Record<string, string>;
}
