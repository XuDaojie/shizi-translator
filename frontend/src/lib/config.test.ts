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
    expect(config.historyLimit).toBe(500);
    expect(config.popupPrecreate).toBe(true);
    expect(config.overlayPrecreate).toBe(false);
    expect(config.collectUsage).toBe(true);
    expect(config.logLevel).toBe('info');
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

  it('投影 historyLimit 时向下取整且保持至少为 1', () => {
    const state = makeState([]);
    state.translation.historyLimit = 1.5;

    const config = projectToAppConfig(state);

    expect(config.historyLimit).toBe(1);
  });

  it('投影翻译行为与提示词字段到后端配置', () => {
    const state = makeState([
      makeInstance({
        id: 'deepseek-1',
        enabled: true,
        apiKey: 'sk-ds',
        systemPrompt: ' 系统 ',
        translationPrompt: ' 翻译 {text} ',
        reflectionPrompt: ' 反思 ',
        reflectionEnabled: true,
        chainOfThought: 'medium',
      }),
    ]);
    state.translation.defaultSourceLang = 'en-US';
    state.translation.autoCopy = false;
    state.translation.restoreClipboard = false;

    const config = projectToAppConfig(state);

    expect(config.defaultSourceLang).toBe('en-US');
    expect(config.autoCopy).toBe(false);
    expect(config.restoreClipboard).toBe(false);
    expect(config.services[0]).toMatchObject({
      systemPrompt: '系统',
      translationPrompt: '翻译 {text}',
      reflectionPrompt: '反思',
      reflectionEnabled: true,
      chainOfThought: 'medium',
    });
  });

  it('投影快捷键绑定到后端 shortcuts 并保留空字符串', () => {
    const state = makeState([]);
    state.shortcut.bindings = [
      { id: 'translate-selection', label: '划词翻译', description: '', keys: ' Alt+D ' },
      { id: 'translate-screenshot', label: '截图翻译', description: '', keys: 'Alt+E' },
      { id: 'word-lookup', label: '取词翻译', description: '', keys: '' },
    ];

    const config = projectToAppConfig(state);

    expect(config.shortcuts).toEqual({
      'translate-selection': 'Alt+D',
      'translate-screenshot': 'Alt+E',
      'word-lookup': '',
    });
  });
});

describe('validateConfig', () => {
  const base: AppConfig = {
    targetLang: '中文',
    defaultSourceLang: 'auto',
    autoCopy: true,
    restoreClipboard: true,
    historyLimit: 500,
    services: [],
    popupPrecreate: true,
    overlayPrecreate: true,
    collectUsage: true,
    logLevel: 'info',
    shortcuts: {},
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
        systemPrompt: '',
        translationPrompt: '',
        reflectionPrompt: '',
        reflectionEnabled: false,
        chainOfThought: 'off',
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
        systemPrompt: '',
        translationPrompt: '',
        reflectionPrompt: '',
        reflectionEnabled: false,
        chainOfThought: 'off',
      }],
    })).toBe('DeepSeek Endpoint 请输入有效的 http(s) 地址');
  });

  it('microsoft_edge 免 Key 渠道允许保存', () => {
    expect(validateConfig({
      ...base,
      services: [{
        id: 'ms-1', serviceType: 'microsoft', name: '微软翻译', enabled: true,
        protocol: 'microsoft_edge', apiKey: null, endpoint: 'https://edge.microsoft.com/translate/translatetext',
        model: '', timeoutSeconds: 60, systemPrompt: '', translationPrompt: '',
        reflectionPrompt: '', reflectionEnabled: false, chainOfThought: 'off',
      }],
    })).toBeNull()
  });
});
