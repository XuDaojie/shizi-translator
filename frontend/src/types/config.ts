// 与后端 src-tauri/src/core/config/types.rs 的 AppConfig 对齐。
// 后端用 #[serde(rename_all = "camelCase")]，故前端字段全部 camelCase。
// 任何一方增删字段，必须同步本文件与 spec、README 配置说明。

export type ServiceProtocolId = 'openai_chat' | 'claude_messages';

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
}

export interface AppConfig {
  targetLang: string;
  services: ServiceInstanceConfig[];
  popupPrecreate: boolean;
  overlayPrecreate: boolean;
  collectUsage: boolean;
}
