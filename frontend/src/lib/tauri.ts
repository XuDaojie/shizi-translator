import type { AppConfig } from '@/types/config';

// 不引 @tauri-apps/api；三页统一走 window.__TAURI__.core.invoke（withGlobalTauri: true）。
const tauriGlobal = (window as unknown as { __TAURI__?: { core: { invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T> } } }).__TAURI__;

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
