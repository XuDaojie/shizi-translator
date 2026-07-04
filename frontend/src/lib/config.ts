import type { AppConfig } from '@/types/config';
import type { AppSettings, ServiceInstance, ShortcutBinding } from '@/settings/types';
import type { Provider } from '@/types/config';

/** 走 OpenAI 兼容协议、后端能用 openai-compatible provider 接入的渠道 type 集合。 */
const OPENAI_COMPATIBLE_TYPES: readonly string[] = ['openai', 'custom', 'deepseek', 'zhipu', 'moonshot', 'siliconflow'];

const DEFAULT_OPENAI = {
  apiKey: null as string | null,
  baseUrl: 'https://api.openai.com/v1',
  model: 'gpt-4o-mini',
  timeoutSeconds: 60,
};
const DEFAULT_CLAUDE = {
  apiKey: null as string | null,
  baseUrl: 'https://api.anthropic.com',
  model: 'claude-haiku-4-5',
  timeoutSeconds: 60,
  enableThinking: false,
};

type ShortcutLike = Pick<ShortcutBinding, 'id' | 'label' | 'keys'>;

const projectShortcuts = (state: AppSettings): Record<string, string> =>
  Object.fromEntries(state.shortcut.bindings.map((binding) => [binding.id, binding.keys.trim()]));

export function validateShortcutBindings(bindings: ShortcutLike[]): Record<string, string> {
  const errors: Record<string, string> = {};
  const seen = new Map<string, ShortcutLike>();

  for (const binding of bindings) {
    const keys = binding.keys.trim();
    if (!keys) continue;

    const normalized = keys.toLowerCase();
    const existing = seen.get(normalized);
    if (existing) {
      errors[binding.id] = `与「${existing.label}」重复`;
      errors[existing.id] ??= `与「${binding.label}」重复`;
    } else {
      seen.set(normalized, binding);
    }
  }

  return errors;
}

/**
 * 校验配置，返回错误文案；无错返回 null。
 * 行为与旧 frontend/settings.js 的 validateConfig 完全一致（逐行平移）。
 */
export function validateConfig(config: AppConfig): string | null {
  if (config.provider === 'mock') return null;
  const sections = config.provider === 'claude' ? [config.claude] : [config.openaiCompatible];
  for (const section of sections) {
    let url: URL;
    try {
      url = new URL(section.baseUrl);
    } catch {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (!section.model) {
      return 'Model 不能为空';
    }
    if (!Number.isInteger(section.timeoutSeconds)
        || section.timeoutSeconds < 1
        || section.timeoutSeconds > 600) {
      return 'Timeout 秒请输入 1-600 的整数';
    }
  }
  return null;
}

/**
 * 把前端 AppSettings(多实例)投影成后端 AppConfig(单 provider)。
 * 按默认实例 type 决定 provider；非支持类型 fallback 到 lastSavedProvider。
 */
export function projectToAppConfig(
  state: AppSettings,
  lastSavedProvider: Provider,
): { config: AppConfig; unsupported: boolean; unsupportedName?: string } {
  const defaultId = state.translation.defaultServiceInstanceId;
  const instance: ServiceInstance | undefined = defaultId
    ? state.services.find((s) => s.id === defaultId)
    : undefined;

  // 默认实例不存在 → 安全降级(无实例可提示)
  if (!instance) {
    return {
      config: makeDefaultConfig(lastSavedProvider, state),
      unsupported: false,
    };
  }

  if (OPENAI_COMPATIBLE_TYPES.includes(instance.type)) {
    return {
      config: makeOpenAiConfig(instance, state),
      unsupported: false,
    };
  }

  if (instance.type === 'claude') {
    return {
      config: makeClaudeConfig(instance, state),
      unsupported: false,
    };
  }

  // 其他 type → fallback：provider 用 lastSavedProvider，段用默认占位
  return {
    config: makeDefaultConfig(lastSavedProvider, state),
    unsupported: true,
    unsupportedName: instance.name,
  };
}

function makeOpenAiConfig(instance: ServiceInstance, state: AppSettings): AppConfig {
  return {
    provider: 'openai-compatible',
    targetLang: state.translation.defaultTargetLang,
    openaiCompatible: {
      apiKey: instance.apiKey || null,
      baseUrl: instance.endpoint || DEFAULT_OPENAI.baseUrl,
      model: instance.model || DEFAULT_OPENAI.model,
      timeoutSeconds: 60,
    },
    claude: { ...DEFAULT_CLAUDE },
    popupPrecreate: state.general.popupPrecreate,
    overlayPrecreate: state.general.overlayPrecreate,
    collectUsage: state.advanced.collectUsage,
    shortcuts: projectShortcuts(state),
  };
}

function makeClaudeConfig(instance: ServiceInstance, state: AppSettings): AppConfig {
  return {
    provider: 'claude',
    targetLang: state.translation.defaultTargetLang,
    openaiCompatible: { ...DEFAULT_OPENAI },
    claude: {
      apiKey: instance.apiKey || null,
      baseUrl: instance.endpoint || DEFAULT_CLAUDE.baseUrl,
      model: instance.model || DEFAULT_CLAUDE.model,
      timeoutSeconds: 60,
      enableThinking: instance.chainOfThought !== 'off',
    },
    popupPrecreate: state.general.popupPrecreate,
    overlayPrecreate: state.general.overlayPrecreate,
    collectUsage: state.advanced.collectUsage,
    shortcuts: projectShortcuts(state),
  };
}

function makeDefaultConfig(provider: Provider, state: AppSettings): AppConfig {
  return {
    provider,
    targetLang: state.translation.defaultTargetLang,
    openaiCompatible: { ...DEFAULT_OPENAI },
    claude: { ...DEFAULT_CLAUDE },
    popupPrecreate: state.general.popupPrecreate,
    overlayPrecreate: state.general.overlayPrecreate,
    collectUsage: state.advanced.collectUsage,
    shortcuts: projectShortcuts(state),
  };
}
