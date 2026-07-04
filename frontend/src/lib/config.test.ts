import { describe, it, expect } from 'vitest';
import { projectToAppConfig, validateConfig } from './config';
import type { AppConfig } from '@/types/config';
import type { AppSettings, ServiceInstance } from '@/settings/types';

const makeInstance = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'inst-1',
  type: 'deepseek',
  name: 'DeepSeek',
  enabled: false,
  protocol: 'openai_chat',
  apiKey: '',
  model: 'deepseek-chat',
  endpoint: 'https://api.deepseek.com',
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

const makeState = (services: ServiceInstance[]): AppSettings => ({
  general: {
    launchAtLogin: false,
    startMinimized: false,
    showTrayIcon: true,
    closeAction: 'minimize',
    theme: 'light',
    language: 'zh-CN',
    updateChannel: 'stable',
    autoCheckUpdate: true,
    popupPrecreate: true,
    overlayPrecreate: false,
  },
  translation: {
    defaultSourceLang: 'auto',
    defaultTargetLang: '中文',
    autoCopy: true,
    restoreClipboard: true,
    autoPaste: false,
    showPhonetic: true,
    showAlternatives: true,
    autoDetect: true,
    wordLookupDelay: 300,
    historyLimit: 500,
  },
  shortcut: { bindings: [] },
  services,
  customServiceTypes: [],
  advanced: { logLevel: 'info', betaLookup: false, betaVoice: false, collectUsage: true },
  ocrHistory: [],
});

describe('projectToAppConfig', () => {
  it('保留 services 数组顺序并投影为后端配置', () => {
    const state = makeState([
      makeInstance({
        id: 'deepseek-1',
        type: 'deepseek',
        name: 'DeepSeek',
        enabled: true,
        apiKey: 'sk-ds',
        endpoint: 'https://api.deepseek.com',
        model: 'deepseek-chat',
      }),
      makeInstance({
        id: 'zhipu-1',
        type: 'zhipu',
        name: '智谱 AI',
        enabled: false,
        apiKey: 'sk-zp',
        endpoint: 'https://open.bigmodel.cn/api/paas/v4',
        model: 'glm-4-flash',
      }),
    ]);

    const config = projectToAppConfig(state);

    expect(config.targetLang).toBe('中文');
    expect(config.popupPrecreate).toBe(true);
    expect(config.overlayPrecreate).toBe(false);
    expect(config.collectUsage).toBe(true);
    expect(config.services.map((s) => s.id)).toEqual(['deepseek-1', 'zhipu-1']);
    expect(config.services[0]).toMatchObject({
      serviceType: 'deepseek',
      name: 'DeepSeek',
      enabled: true,
      protocol: 'openai_chat',
      apiKey: 'sk-ds',
      endpoint: 'https://api.deepseek.com',
      model: 'deepseek-chat',
      timeoutSeconds: 60,
    });
  });
});

describe('validateConfig', () => {
  const base: AppConfig = {
    targetLang: '中文',
    services: [],
    popupPrecreate: true,
    overlayPrecreate: true,
    collectUsage: true,
  };

  it('没有启用服务时允许保存', () => {
    expect(validateConfig(base)).toBeNull();
  });

  it('启用服务缺 API Key 时报错', () => {
    expect(validateConfig({
      ...base,
      services: [{
        id: 'deepseek-1',
        serviceType: 'deepseek',
        name: 'DeepSeek',
        enabled: true,
        protocol: 'openai_chat',
        apiKey: null,
        endpoint: 'https://api.deepseek.com',
        model: 'deepseek-chat',
        timeoutSeconds: 60,
      }],
    })).toBe('DeepSeek 请先填写 API Key');
  });

  it('启用服务 endpoint 必须是 http(s)', () => {
    expect(validateConfig({
      ...base,
      services: [{
        id: 'deepseek-1',
        serviceType: 'deepseek',
        name: 'DeepSeek',
        enabled: true,
        protocol: 'openai_chat',
        apiKey: 'sk-x',
        endpoint: 'ftp://api.deepseek.com',
        model: 'deepseek-chat',
        timeoutSeconds: 60,
      }],
    })).toBe('DeepSeek Endpoint 请输入有效的 http(s) 地址');
  });
});
