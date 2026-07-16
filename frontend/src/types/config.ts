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

export interface AppConfig {
  interfaceLanguage: string;
  targetLang: string;
  defaultSourceLang: string;
  autoCopy: boolean;
  restoreClipboard: boolean;
  historyLimit: number;
  services: ServiceInstanceConfig[];
  ocrServices: OcrServiceInstanceConfig[];
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
  logLevel: LogLevel;
  updateChannel: 'stable' | 'beta';
  autoCheckUpdate: boolean;
  shortcuts: Record<string, string>;
}
