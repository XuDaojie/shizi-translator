import type { AppConfig, ServiceProtocolId } from '@/types/config';
import type { AppSettings } from '@/settings/types';

const AVAILABLE_PROTOCOLS: readonly ServiceProtocolId[] = ['openai_chat', 'claude_messages'];

export function validateConfig(config: AppConfig): string | null {
  for (const service of config.services.filter((s) => s.enabled)) {
    if (!AVAILABLE_PROTOCOLS.includes(service.protocol)) {
      return `${service.name} 当前协议不可用`;
    }
    if (!service.apiKey?.trim()) {
      return `${service.name} 请先填写 API Key`;
    }
    let url: URL;
    try {
      url = new URL(service.endpoint);
    } catch {
      return `${service.name} Endpoint 请输入有效的 http(s) 地址`;
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return `${service.name} Endpoint 请输入有效的 http(s) 地址`;
    }
    if (!service.model.trim()) {
      return `${service.name} Model 不能为空`;
    }
    if (!Number.isInteger(service.timeoutSeconds)
      || service.timeoutSeconds < 1
      || service.timeoutSeconds > 600) {
      return `${service.name} Timeout 秒请输入 1-600 的整数`;
    }
  }
  return null;
}

export function projectToAppConfig(state: AppSettings): AppConfig {
  return {
    targetLang: state.translation.defaultTargetLang,
    defaultSourceLang: state.translation.defaultSourceLang,
    autoCopy: state.translation.autoCopy,
    restoreClipboard: state.translation.restoreClipboard,
    services: state.services.map((service) => ({
      id: service.id,
      serviceType: service.type,
      name: service.name,
      enabled: service.enabled,
      protocol: service.protocol,
      apiKey: service.apiKey.trim() || null,
      endpoint: service.endpoint.trim(),
      model: service.model.trim(),
      // ponytail: 固定 60s，等 UI 暴露 timeout 字段后从 service 读取
      timeoutSeconds: 60,
      systemPrompt: service.systemPrompt.trim(),
      translationPrompt: service.translationPrompt.trim(),
      reflectionPrompt: service.reflectionPrompt.trim(),
      reflectionEnabled: service.reflectionEnabled,
      chainOfThought: service.chainOfThought,
    })),
    popupPrecreate: state.general.popupPrecreate,
    overlayPrecreate: state.general.overlayPrecreate,
    collectUsage: state.advanced.collectUsage,
    logLevel: state.advanced.logLevel,
    shortcuts: Object.fromEntries(
      state.shortcut.bindings.map((binding) => [binding.id, binding.keys.trim()]),
    ),
  };
}
