import type { AppConfig } from '@/types/config';

// 不引 @tauri-apps/api；三页统一走 window.__TAURI__.core.invoke（withGlobalTauri: true）。
const tauriGlobal = typeof window === 'undefined'
  ? undefined
  : (window as unknown as { __TAURI__?: { core: { invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T> } } }).__TAURI__;

function requireInvoke() {
  const invoke = tauriGlobal?.core?.invoke;
  if (!invoke) {
    throw new Error('Tauri API 未就绪');
  }
  return invoke;
}

export async function invokeGetAppConfig(): Promise<AppConfig> {
  return requireInvoke()<AppConfig>('get_app_config');
}

export async function invokeSaveAppConfig(config: AppConfig): Promise<AppConfig> {
  return requireInvoke()<AppConfig>('save_app_config', { config });
}

/** 启动时快捷键注册失败的冲突项（id 对应 ShortcutBinding.id）。 */
export interface ShortcutConflict {
  id: string
  message: string
}

export async function invokeGetShortcutConflicts(): Promise<ShortcutConflict[]> {
  return requireInvoke()<ShortcutConflict[]>('get_shortcut_conflicts')
}

export async function invokeOpenSettings(): Promise<void> {
  return requireInvoke()<void>('open_settings')
}

/** 供组件层判断是否就绪（用于挂载时给出"Tauri API 未就绪"提示）。 */
export function isTauriReady(): boolean {
  return Boolean(tauriGlobal?.core?.invoke);
}

export interface ServiceProbeRequest {
  protocol: string;
  endpoint: string;
  apiKey: string | null;
}

export async function invokeValidateServiceCredential(request: ServiceProbeRequest): Promise<void> {
  return requireInvoke()<void>('validate_service_credential', { request });
}

export async function invokeListServiceModels(request: ServiceProbeRequest): Promise<{ models: string[] }> {
  return requireInvoke()<{ models: string[] }>('list_service_models', { request });
}

export interface FrontendLogEntry {
  level: 'error' | 'warn' | 'info' | 'debug';
  message: string;
  timestamp: string;
  source: string;
  meta?: unknown;
}

export async function invokeWriteFrontendLog(entries: FrontendLogEntry[]): Promise<void> {
  return requireInvoke()<void>('write_frontend_log', { entries });
}

export async function invokeExportLogs(): Promise<string> {
  return requireInvoke()<string>('export_logs');
}

export type HistoryTrigger = 'selection' | 'manual' | 'screenshot'
export type HistoryResultStatus = 'success' | 'error' | 'cancelled' | 'pending'

export interface HistoryResultDto {
  serviceInstanceId: string
  serviceName: string
  serviceType: string
  protocol: string
  modelName: string
  translation: string
  errorMessage: string
  status: HistoryResultStatus
  inputTokens: number | null
  outputTokens: number | null
}

export interface HistorySessionDto {
  id: string
  timestamp: string
  trigger: HistoryTrigger
  sourceLang: string
  targetLang: string
  source: string
  results: HistoryResultDto[]
}

export async function invokeListTranslationHistory(limit?: number): Promise<HistorySessionDto[]> {
  return requireInvoke()<HistorySessionDto[]>('list_translation_history', { limit })
}

export async function invokeClearTranslationHistory(): Promise<void> {
  return requireInvoke()<void>('clear_translation_history')
}

export interface LanguageMeta {
  locale: string
  name: string
  builtin: boolean
}

export interface LanguagePackError {
  file: string
  message: string
}

export interface InterfaceLanguageSnapshot {
  configuredLocale: string
  locale: string
  revision: number
  languages: LanguageMeta[]
  userMessages: Record<string, string>
  errors: LanguagePackError[]
}

export async function invokeGetInterfaceLanguageSnapshot(): Promise<InterfaceLanguageSnapshot> {
  return requireInvoke()<InterfaceLanguageSnapshot>('get_interface_language_snapshot')
}

export async function invokeRefreshInterfaceLanguages(): Promise<InterfaceLanguageSnapshot> {
  return requireInvoke()<InterfaceLanguageSnapshot>('refresh_interface_languages')
}

export async function invokeOpenLanguagePackDirectory(): Promise<void> {
  return requireInvoke()<void>('open_language_pack_directory')
}
