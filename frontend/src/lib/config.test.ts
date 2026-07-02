import { describe, it, expect } from 'vitest';
import { validateConfig } from './config';
import type { AppConfig } from '@/types/config';
import { projectToAppConfig } from './config';
import type { AppSettings, ServiceInstance } from '@/settings/types';

const base: AppConfig = {
  provider: 'openai-compatible',
  targetLang: '中文',
  openaiCompatible: { apiKey: 'sk-x', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', timeoutSeconds: 60 },
  claude: { apiKey: null, baseUrl: 'https://api.anthropic.com', model: 'claude-haiku-4-5', timeoutSeconds: 60, enableThinking: false },
  popupPrecreate: true,
  overlayPrecreate: true,
  collectUsage: true,
};

describe('validateConfig', () => {
  it('mock provider 跳过校验', () => {
    expect(validateConfig({ ...base, provider: 'mock' })).toBeNull();
  });

  it('openai-compatible 有效配置返回 null', () => {
    expect(validateConfig(base)).toBeNull();
  });

  it('baseUrl 非 http(s) 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, baseUrl: 'ftp://x' } }))
      .toBe('Base URL 请输入有效的 http(s) 地址');
  });

  it('baseUrl 非法 URL 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, baseUrl: 'not a url' } }))
      .toBe('Base URL 请输入有效的 http(s) 地址');
  });

  it('model 为空报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, model: '' } }))
      .toBe('Model 不能为空');
  });

  it('timeoutSeconds 小于 1 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 0 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('timeoutSeconds 大于 600 报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 601 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('timeoutSeconds 非整数报错', () => {
    expect(validateConfig({ ...base, openaiCompatible: { ...base.openaiCompatible, timeoutSeconds: 1.5 } }))
      .toBe('Timeout 秒请输入 1-600 的整数');
  });

  it('claude provider 校验 claude 段', () => {
    const c = { ...base, provider: 'claude' as const, claude: { ...base.claude, model: '' } };
    expect(validateConfig(c)).toBe('Model 不能为空');
  });

  it('claude provider 有效配置返回 null', () => {
    const c = { ...base, provider: 'claude' as const, claude: { ...base.claude, apiKey: 'sk-ant-x' } };
    expect(validateConfig(c)).toBeNull();
  });
});

// === projectToAppConfig === //

const defaultOpenai = 'https://api.openai.com/v1';
const defaultClaude = 'https://api.anthropic.com';

const makeInstance = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'inst-1',
  type: 'openai',
  name: 'OpenAI',
  enabled: true,
  apiKey: '',
  model: 'gpt-4o-mini',
  endpoint: '',
  note: '',
  pulledModels: [],
  keyStatus: 'idle',
  chainOfThought: 'off',
  systemPrompt: '',
  translationPrompt: '',
  reflectionPrompt: '',
  reflectionEnabled: false,
  ...over,
});

const makeState = (services: ServiceInstance[], defaultId: string): AppSettings => ({
  general: {
    launchAtLogin: false, startMinimized: false, showTrayIcon: true,
    closeAction: 'minimize', theme: 'light', language: 'zh-CN',
    updateChannel: 'stable', autoCheckUpdate: true,
    popupPrecreate: true, overlayPrecreate: false,
  },
  translation: {
    defaultSourceLang: 'auto', defaultTargetLang: '中文',
    defaultServiceInstanceId: defaultId,
    autoCopy: true, restoreClipboard: true, autoPaste: false,
    showPhonetic: true, showAlternatives: true, autoDetect: true,
    wordLookupDelay: 300, historyLimit: 500,
  },
  shortcut: { bindings: [] },
  services,
  customServiceTypes: [],
  advanced: { logLevel: 'info', betaLookup: false, betaVoice: false, collectUsage: true },
  ocrHistory: [],
});

describe('projectToAppConfig', () => {
  it('默认实例为 openai → provider=openai-compatible，字段取实例值', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'openai', apiKey: 'sk-x', endpoint: 'https://api.openai.com/v1', model: 'gpt-4o' })],
      'i1',
    );
    const { config, unsupported } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(false);
    expect(config.provider).toBe('openai-compatible');
    expect(config.openaiCompatible.apiKey).toBe('sk-x');
    expect(config.openaiCompatible.baseUrl).toBe('https://api.openai.com/v1');
    expect(config.openaiCompatible.model).toBe('gpt-4o');
    expect(config.openaiCompatible.timeoutSeconds).toBe(60);
  });

  it('默认实例为 claude → provider=claude，enableThinking 按 chainOfThought 映射', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'claude', apiKey: 'sk-ant', endpoint: defaultClaude, model: 'claude-haiku-4-5', chainOfThought: 'medium' })],
      'i1',
    );
    const { config } = projectToAppConfig(s, 'openai-compatible');
    expect(config.provider).toBe('claude');
    expect(config.claude.apiKey).toBe('sk-ant');
    expect(config.claude.enableThinking).toBe(true);
  });

  it('默认实例 chainOfThought=off → enableThinking=false', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'claude', chainOfThought: 'off' })],
      'i1',
    );
    const { config } = projectToAppConfig(s, 'openai-compatible');
    expect(config.claude.enableThinking).toBe(false);
  });

  it('默认实例为 deepseek（openai 兼容集）→ provider=openai-compatible', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'deepseek', apiKey: 'sk-ds', endpoint: 'https://api.deepseek.com/v1', model: 'deepseek-chat' })],
      'i1',
    );
    const { config, unsupported } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(false);
    expect(config.provider).toBe('openai-compatible');
    expect(config.openaiCompatible.model).toBe('deepseek-chat');
  });

  it('默认实例为 deepl（非支持类型）→ fallback 到 lastSavedProvider 且 unsupported=true', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'deepl', name: 'DeepL', apiKey: 'k' })],
      'i1',
    );
    const { config, unsupported, unsupportedName } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(true);
    expect(unsupportedName).toBe('DeepL');
    expect(config.provider).toBe('openai-compatible');
    expect(config.openaiCompatible.apiKey).toBeNull();
    expect(config.openaiCompatible.baseUrl).toBe(defaultOpenai);
    expect(config.openaiCompatible.model).toBe('gpt-4o-mini');
  });

  it('fallback 时 lastSavedProvider=claude → provider=claude，claude 段用默认占位', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'deepl', name: 'DeepL' })],
      'i1',
    );
    const { config } = projectToAppConfig(s, 'claude');
    expect(config.provider).toBe('claude');
    expect(config.claude.baseUrl).toBe(defaultClaude);
    expect(config.claude.model).toBe('claude-haiku-4-5');
  });

  it('默认实例不存在（id 空）→ 安全降级，provider=lastSavedProvider，unsupported=false', () => {
    const s = makeState([], '');
    const { config, unsupported } = projectToAppConfig(s, 'openai-compatible');
    expect(unsupported).toBe(false);
    expect(config.provider).toBe('openai-compatible');
    expect(config.openaiCompatible.baseUrl).toBe(defaultOpenai);
  });

  it('已实现字段 targetLang/popupPrecreate/overlayPrecreate/collectUsage 透传', () => {
    const s = makeState(
      [makeInstance({ id: 'i1', type: 'openai' })],
      'i1',
    );
    s.translation.defaultTargetLang = 'English';
    s.general.popupPrecreate = false;
    s.general.overlayPrecreate = true;
    s.advanced.collectUsage = false;
    const { config } = projectToAppConfig(s, 'openai-compatible');
    expect(config.targetLang).toBe('English');
    expect(config.popupPrecreate).toBe(false);
    expect(config.overlayPrecreate).toBe(true);
    expect(config.collectUsage).toBe(false);
  });
});
